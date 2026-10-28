use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

use crossbeam_channel::Receiver;
use hololive_core::{
    DisplayMode, Frame, Real, Reconstruction, ReconstructionEngine, ReconstructionSettings,
    ReferenceWave,
};

use crate::{
    export,
    levels::{self, DisplayLevels, GridSpec},
};

use super::{
    FrameSample, LiveResult, LiveView, PipelineEvent, ReconstructionCommand, latest::LatestSlot,
    status::RuntimeStatus,
};

pub(crate) struct ReconstructionWorker<T> {
    pub repaint: Arc<dyn Fn() + Send + Sync>,
    pub frames: Arc<LatestSlot<Frame<T>>>,
    pub results: Arc<LatestSlot<LiveResult>>,
    pub settings: Arc<LatestSlot<ReconstructionSettings>>,
    pub commands: Receiver<ReconstructionCommand>,
    pub recycle: Receiver<LiveResult>,
    pub events: Arc<LatestSlot<PipelineEvent>>,
    pub status: Arc<Mutex<RuntimeStatus>>,
    pub paused: Arc<AtomicBool>,
    pub cancelled: Arc<AtomicBool>,
    pub initial_settings: ReconstructionSettings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
enum Work {
    #[default]
    Idle,
    Pack,
    Reconstruct,
}

pub(crate) fn spawn_reconstruction<T: FrameSample>(
    worker: ReconstructionWorker<T>,
) -> JoinHandle<()> {
    let ReconstructionWorker {
        repaint,
        frames,
        results,
        settings: settings_slot,
        commands,
        recycle,
        events,
        status,
        paused,
        cancelled,
        initial_settings,
    } = worker;
    thread::Builder::new()
        .name("hololive-reconstruction".into())
        .spawn(move || {
            let mut engine = ReconstructionEngine::<T>::default();
            let mut settings = initial_settings;
            let mut reference: Option<ReferenceWave<T>> = None;
            let mut output = Reconstruction::default();
            let mut live = LiveResult::default();
            let mut packer = LiveViewPacker::default();
            let mut latest_frame = None;
            let mut frame_size = None;
            let mut revision = 0;
            let mut work = Work::Idle;
            let mut capture = false;
            let mut have_wave = false;
            let mut export_worker = None;
            let mut view = LiveView::Hologram;
            loop {
                for command in commands.try_iter() {
                    match command {
                        ReconstructionCommand::Settings(next) => {
                            settings = next;
                            work = Work::Reconstruct;
                        }
                        ReconstructionCommand::View(next) => {
                            if view != next {
                                view = next;
                                if latest_frame.is_some() {
                                    work = work.max(Work::Pack);
                                }
                            }
                        }
                        ReconstructionCommand::CaptureReference => {
                            capture = true;
                            work = Work::Reconstruct;
                        }
                        ReconstructionCommand::ClearReference => {
                            capture = false;
                            clear_reference(&mut reference, &status);
                            work = Work::Reconstruct;
                        }
                        ReconstructionCommand::Save => {
                            save_wave(&output, have_wave, &mut export_worker, &events, &repaint)
                        }
                    }
                }
                if cancelled.load(Ordering::Acquire) {
                    break;
                }
                if let Some(next) = settings_slot.take() {
                    settings = next;
                    work = Work::Reconstruct;
                }
                take_live_frame(&paused, &frames, &mut latest_frame, &mut work);
                if latest_frame.is_none() || work == Work::Idle {
                    frames.wait_for_change(&mut revision, &cancelled);
                    continue;
                }
                take_live_frame(&paused, &frames, &mut latest_frame, &mut work);
                let reconstruct = work == Work::Reconstruct;
                work = Work::Idle;
                let frame = latest_frame.as_ref().expect("checked latest frame");
                while let Ok(recycled) = recycle.try_recv() {
                    live = recycled;
                }
                if reconstruct {
                    let size = (frame.width(), frame.height());
                    if frame_size
                        .replace(size)
                        .is_some_and(|previous| previous != size)
                    {
                        clear_reference(&mut reference, &status);
                        settings.locked_sideband = None;
                        settings.crop_radius = None;
                        capture = false;
                    }
                    // Unlocked snapshots can predate capture; explicit unlocks clear the reference.
                    if let Some(reference) = &reference
                        && settings.locked_sideband.is_none()
                    {
                        settings.locked_sideband = Some(reference.settings.sideband);
                        settings.crop_radius = Some(reference.settings.crop_size / 2);
                    }
                    let mut reconstructed = engine.reconstruct_retained_into(
                        frame,
                        &settings,
                        reference.as_ref(),
                        &mut output,
                    );
                    if capture && reconstructed.is_ok() {
                        capture = false;
                        adopt_captured_reference(&engine, &mut reference, &mut settings, &status);
                        reconstructed = engine.reconstruct_retained_into(
                            frame,
                            &settings,
                            reference.as_ref(),
                            &mut output,
                        );
                    }
                    have_wave = reconstructed.is_ok();
                    if let Err(error) = reconstructed {
                        if matches!(error, hololive_core::CoreError::IncompatibleReference) {
                            clear_reference(&mut reference, &status);
                            work = Work::Reconstruct;
                        }
                        let _superseded =
                            events.publish(PipelineEvent::ReconstructionFailed(error));
                        repaint();
                    }
                }
                if view == LiveView::Reconstruction && !have_wave {
                    live.rgba.clear();
                    live.display_width = 0;
                    live.display_height = 0;
                } else {
                    packer.pack(view, &engine, frame, &output, &mut live);
                }
                live.view = view;
                live.acquired_unix_ns = frame.meta.acquired_unix_ns;
                live.reconstruction = have_wave.then(|| output.info());
                live = results.publish(live).unwrap_or_default();
                repaint();
            }
            if let Some(worker) = export_worker {
                let _ = worker.join();
            }
        })
        .expect("failed to start reconstruction thread")
}

fn take_live_frame<T>(
    paused: &AtomicBool,
    frames: &LatestSlot<Frame<T>>,
    latest_frame: &mut Option<Frame<T>>,
    work: &mut Work,
) {
    if paused.load(Ordering::Acquire) {
        return;
    }
    if let Some(frame) = frames.take() {
        *latest_frame = Some(frame);
        *work = Work::Reconstruct;
    }
}

#[derive(Default)]
struct LiveViewPacker {
    levels: DisplayLevels,
    spectrum: Vec<f32>,
}

impl LiveViewPacker {
    fn pack<T: Real>(
        &mut self,
        view: LiveView,
        engine: &ReconstructionEngine<T>,
        frame: &Frame<T>,
        output: &Reconstruction<T>,
        live: &mut LiveResult,
    ) {
        let sequence = frame.meta.sequence;
        let (width, height, nearest) = match view {
            LiveView::Hologram => {
                let range = self.levels.hologram(
                    frame.pixels(),
                    GridSpec {
                        width: frame.width(),
                        height: frame.height(),
                        sequence,
                    },
                );
                levels::pack_rgba_into(frame.pixels(), range, &mut live.rgba);
                (frame.width(), frame.height(), false)
            }
            LiveView::Spectrum => {
                let (width, height) = engine
                    .write_spectrum(&mut self.spectrum)
                    .expect("retained detector frame has a spectrum");
                let range = self.levels.spectrum(
                    &self.spectrum,
                    GridSpec {
                        width,
                        height,
                        sequence,
                    },
                );
                levels::pack_rgba_into(&self.spectrum, range, &mut live.rgba);
                self.spectrum.clear();
                (width, height, false)
            }
            LiveView::Reconstruction => {
                let wrapped_phase =
                    output.display_mode == DisplayMode::Phase && output.unwrapped_range.is_none();
                let range = match output.display_mode {
                    DisplayMode::Phase => output
                        .unwrapped_range
                        .unwrap_or_else(levels::phase_wrap_range),
                    DisplayMode::Amplitude => self.levels.amplitude(
                        output.values.as_slice(),
                        GridSpec {
                            width: output.width,
                            height: output.height,
                            sequence,
                        },
                    ),
                };
                levels::pack_rgba_into(&output.values, range, &mut live.rgba);
                (output.width, output.height, wrapped_phase)
            }
        };
        live.display_width = width;
        live.display_height = height;
        live.nearest = nearest;
    }
}

fn clear_reference<T>(reference: &mut Option<ReferenceWave<T>>, status: &Mutex<RuntimeStatus>) {
    *reference = None;
    status
        .lock()
        .expect("status lock poisoned")
        .clear_reference();
}

fn adopt_captured_reference<T: Real>(
    engine: &ReconstructionEngine<T>,
    reference: &mut Option<ReferenceWave<T>>,
    settings: &mut ReconstructionSettings,
    status: &Mutex<RuntimeStatus>,
) {
    let Some(captured) = engine.capture_reference() else {
        return;
    };
    let crop_radius = (captured.settings.crop_size / 2).max(1);
    settings.locked_sideband = Some(captured.settings.sideband);
    settings.crop_radius = Some(crop_radius);
    status
        .lock()
        .expect("status lock poisoned")
        .capture_reference(
            captured.settings.sideband,
            crop_radius,
            (
                captured.settings.input_width,
                captured.settings.input_height,
            ),
        );
    *reference = Some(captured);
}

fn save_wave<T: Real>(
    output: &Reconstruction<T>,
    have_wave: bool,
    worker: &mut Option<JoinHandle<()>>,
    events: &Arc<LatestSlot<PipelineEvent>>,
    repaint: &Arc<dyn Fn() + Send + Sync>,
) {
    if !have_wave {
        let _superseded = events.publish(PipelineEvent::SaveFinished(Err(
            "Nothing To Save Yet".into()
        )));
        repaint();
        return;
    }
    if worker.as_ref().is_some_and(|worker| !worker.is_finished()) {
        return;
    }
    if let Some(previous) = worker.take() {
        let _ = previous.join();
    }
    let values = output.values.clone();
    let width = output.width;
    let height = output.height;
    let display_mode = output.display_mode;
    let events = Arc::clone(events);
    let repaint = Arc::clone(repaint);
    *worker = Some(
        thread::Builder::new()
            .name("hololive-export".into())
            .spawn(move || {
                let result = export::save(&values, width, height, display_mode)
                    .map(|path| format!("Saved {}", path.display()))
                    .map_err(|error| error.to_string());
                let _superseded = events.publish(PipelineEvent::SaveFinished(result));
                repaint();
            })
            .expect("failed to start export thread"),
    );
}
