//! `cargo xtask fixtures import`: cut one note out of a source archive into `fixtures/audio/`.
//!
//! The cut keeps the source's rate, depth and samples exactly: one channel, trimmed on frame
//! boundaries, never resampled, mixed, normalised, dithered or faded (`docs/adr/0030`). The
//! archive stays outside the repo; its hash, the member's and the output's go in the manifest.

use std::fs::File;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

use super::Args;
use super::manifest::{self, Channel, Fixture, Set};

/// Run `import` with its `--key value` arguments.
pub fn run(args: &Args) -> Result<(), String> {
    let root = manifest::dir();
    let archive = PathBuf::from(args.get("archive")?);
    let member = args.get("member")?;
    let id = args.get("id")?;
    let source_id = args.get("source")?;
    let channel = parse_channel(args.get("channel")?)?;
    let set = parse_set(args.get("set")?)?;
    let window = [args.number("start-ms")?, args.number("end-ms")?];
    if window[1] <= window[0] {
        return Err("--end-ms must be after --start-ms".into());
    }

    let mut manifest = manifest::load(&root)?;
    let source = manifest
        .sources
        .get_mut(source_id)
        .ok_or_else(|| format!("--source {source_id} is not in the manifest's sources"))?;
    if archive.file_name().and_then(|n| n.to_str()) != Some(source.archive.as_str()) {
        return Err(format!(
            "{} is not the archive `{source_id}` names ({})",
            archive.display(),
            source.archive
        ));
    }
    let archive_sha256 = sha256_file(&archive)?;
    if source.archive_sha256.is_empty() {
        source.archive_sha256.clone_from(&archive_sha256);
    } else if source.archive_sha256 != archive_sha256 {
        return Err(format!(
            "{} does not match the archive the manifest pins",
            archive.display()
        ));
    }

    let bytes = extract(&archive, member)?;
    let cut = cut(&bytes, member, channel, window)?;
    let file = format!("{id}.wav");
    let mut writer =
        hound::WavWriter::create(root.join(&file), cut.spec).map_err(|e| e.to_string())?;
    for &sample in &cut.samples {
        writer.write_sample(sample).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())?;
    let sha256 = manifest::sha256_hex(&std::fs::read(root.join(&file)).map_err(|e| e.to_string())?);

    // A new cut invalidates the old reference, unless the bytes are the same.
    let previous = manifest.fixtures.iter().position(|f| f.id == id);
    let reference = previous
        .map(|i| &manifest.fixtures[i])
        .filter(|f| f.sha256 == sha256)
        .and_then(|f| f.reference.clone());
    let frames = u64::try_from(cut.samples.len()).map_err(|e| e.to_string())?;
    let fixture = Fixture {
        id: id.to_owned(),
        file,
        source: source_id.to_owned(),
        member: member.to_owned(),
        member_sha256: manifest::sha256_hex(&bytes),
        channel,
        start_frame: cut.start_frame,
        frames,
        instrument: args.get("instrument")?.to_owned(),
        variant: args.get("variant")?.to_owned(),
        set,
        note: args.get("note")?.to_owned(),
        sample_rate: cut.spec.sample_rate,
        bits: cut.spec.bits_per_sample,
        sha256,
        reference,
    };
    match previous {
        Some(i) => manifest.fixtures[i] = fixture,
        None => manifest.fixtures.push(fixture),
    }
    manifest::save(&root, &mut manifest)?;
    println!(
        "imported {id}: {frames} frames at {} Hz, {}-bit",
        cut.spec.sample_rate, cut.spec.bits_per_sample
    );
    Ok(())
}

/// One channel of a source WAV between two times, and the mono spec it is written with.
struct Cut {
    spec: hound::WavSpec,
    start_frame: u64,
    samples: Vec<i32>,
}

fn cut(bytes: &[u8], member: &str, channel: Channel, window_ms: [u64; 2]) -> Result<Cut, String> {
    let mut reader =
        hound::WavReader::new(Cursor::new(bytes)).map_err(|e| format!("{member}: {e}"))?;
    let spec = reader.spec();
    if spec.sample_format != hound::SampleFormat::Int {
        return Err(format!("{member}: float WAV; fixtures are integer PCM"));
    }
    if spec.sample_rate != 44_100 && spec.sample_rate != 48_000 {
        return Err(format!(
            "{member}: {} Hz; fixtures are 44.1 or 48 kHz",
            spec.sample_rate
        ));
    }
    let channels = usize::from(spec.channels);
    let pick = match (channel, channels) {
        (Channel::Mono, 1) | (Channel::Left, 2) => 0,
        (Channel::Right, 2) => 1,
        _ => {
            return Err(format!(
                "{member}: --channel does not fit a {channels}-channel source"
            ));
        }
    };
    let interleaved: Vec<i32> = reader
        .samples::<i32>()
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let total = interleaved.len() / channels;
    let rate = u64::from(spec.sample_rate);
    let to_frame = |ms: u64| usize::try_from(ms * rate / 1000).map_err(|e| e.to_string());
    let (first, last) = (to_frame(window_ms[0])?, to_frame(window_ms[1])?.min(total));
    if first >= last {
        return Err(format!(
            "{member}: the cut starts after the source ends ({total} frames)"
        ));
    }
    let samples = interleaved[first * channels..last * channels]
        .chunks_exact(channels)
        .map(|frame| frame[pick])
        .collect();
    Ok(Cut {
        spec: hound::WavSpec {
            channels: 1,
            ..spec
        },
        start_frame: u64::try_from(first).map_err(|e| e.to_string())?,
        samples,
    })
}

fn parse_channel(text: &str) -> Result<Channel, String> {
    match text {
        "mono" => Ok(Channel::Mono),
        "left" => Ok(Channel::Left),
        "right" => Ok(Channel::Right),
        other => Err(format!("--channel {other}: expected mono, left or right")),
    }
}

fn parse_set(text: &str) -> Result<Set, String> {
    match text {
        "gold" => Ok(Set::Gold),
        "ceiling" => Ok(Set::Ceiling),
        "stress" => Ok(Set::Stress),
        other => Err(format!("--set {other}: expected gold, ceiling or stress")),
    }
}

/// One member's bytes, by the archive tool its extension names. Authoring only, so the tools are
/// the host's: `unzip` and `7z`.
fn extract(archive: &Path, member: &str) -> Result<Vec<u8>, String> {
    let name = archive.to_string_lossy();
    let extension = archive
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    let mut command = match extension.as_deref() {
        Some("zip") => {
            let mut c = Command::new("unzip");
            c.arg("-p").arg(archive).arg(member);
            c
        }
        Some("7z") => {
            let mut c = Command::new("7z");
            c.args(["e", "-so"]).arg(archive).arg(member);
            c
        }
        _ => return Err(format!("{name}: expected a .zip or .7z archive")),
    };
    let output = command.output().map_err(|e| format!("{name}: {e}"))?;
    if !output.status.success() || output.stdout.is_empty() {
        return Err(format!("{name}: no member `{member}`"));
    }
    Ok(output.stdout)
}

/// SHA-256 of a file of any size, streamed: VSCO's archive is 2.3 GB.
fn sha256_file(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let mut file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1 << 20];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(manifest::hex(&hasher.finalize()))
}
