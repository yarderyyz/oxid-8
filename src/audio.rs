use std::f32::consts::PI;

use color_eyre::eyre::{bail, eyre, Result};
use cpal::traits::{DeviceTrait, HostTrait};

pub fn setup() -> Result<cpal::Stream> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| eyre!("No output device"))?;
    let supported = device.default_output_config()?;

    // Capture sample format before converting to StreamConfig
    let sample_format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();

    let sample_rate = config.sample_rate.0 as f32;
    let channels = config.channels as usize;

    // Tone params
    let freq = 440.0_f32;
    let amp = 0.2_f32;
    let mut phase = 0.0_f32;
    let phase_inc = 2.0 * PI * freq / sample_rate;

    let err_fn = |e| eprintln!("stream error: {e}");

    let stream = match sample_format {
        cpal::SampleFormat::F32 => device.build_output_stream(
            &config,
            move |data: &mut [f32], _| write_sine(data, channels, amp, &mut phase, phase_inc),
            err_fn,
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config,
            move |data: &mut [i16], _| write_sine_i16(data, channels, amp, &mut phase, phase_inc),
            err_fn,
            None,
        )?,
        cpal::SampleFormat::U16 => device.build_output_stream(
            &config,
            move |data: &mut [u16], _| write_sine_u16(data, channels, amp, &mut phase, phase_inc),
            err_fn,
            None,
        )?,
        other => bail!("Unsupported sample format: {other:?}"),
    };

    Ok(stream)
}

fn write_sine(buf: &mut [f32], ch: usize, amp: f32, phase: &mut f32, inc: f32) {
    for frame in buf.chunks_mut(ch) {
        let s = (*phase).sin() * amp;
        *phase = (*phase + inc) % (2.0 * PI);
        for sample in frame {
            *sample = s;
        }
    }
}

fn write_sine_i16(buf: &mut [i16], ch: usize, amp: f32, phase: &mut f32, inc: f32) {
    for frame in buf.chunks_mut(ch) {
        let f = (*phase).sin() * amp;
        *phase = (*phase + inc) % (2.0 * PI);
        let s = (f * i16::MAX as f32) as i16;
        for sample in frame {
            *sample = s;
        }
    }
}

// Map [-amp, amp] -> [0, 1] then to u16 range
fn write_sine_u16(buf: &mut [u16], ch: usize, amp: f32, phase: &mut f32, inc: f32) {
    for frame in buf.chunks_mut(ch) {
        let f = (*phase).sin() * amp;
        *phase = (*phase + inc) % (2.0 * PI);
        let s = ((f * 0.5 + 0.5) * u16::MAX as f32) as u16;
        for sample in frame {
            *sample = s;
        }
    }
}
