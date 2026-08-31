//! Locally installed, fixed-command X11 implementation. No application adapters.
//! scrot 1.10's -a path calls imlib_create_image_from_drawable for this rectangle
//! directly; neither this process nor scrot persists an uncropped desktop frame.
use crate::modality::{CancellationToken, acquisition::*};
use std::{
    fs::File,
    io::{self, Read},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub struct HostStaticVisualProvider;

impl StaticVisualAcquisitionProvider for HostStaticVisualProvider {
    fn capabilities(&self) -> AcquisitionCapabilities {
        AcquisitionCapabilities {
            available: cfg!(target_os = "linux")
                && std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("x11"),
            quality: CaptureQuality::CompositedScreenSnapshot,
            limitation: "single native X11 screen, identity transform, 96 DPI; occlusion is not excluded; Wayland unavailable",
        }
    }

    fn acquire(
        &self,
        request: SemanticVisualRegion,
        cancel: &CancellationToken,
    ) -> io::Result<AcquiredVisual> {
        if cancel.is_cancelled() {
            return Err(io::Error::other(
                "static acquisition cancelled before provider invocation",
            ));
        }
        if !self.capabilities().available {
            return Err(io::Error::other(
                "AcquisitionUnavailable: no supported static provider (native X11 required)",
            ));
        }
        let source = source_geometry(cancel)?;
        let region = request.bounds;
        region.validate(source.0, source.1)?;
        verify_window(request, cancel)?;
        // Private temporary directory contains only the requested cropped PNG.
        // Drop cleans partial data on every failure/cancellation path.
        let mut builder = tempfile::Builder::new();
        builder.prefix("gui2tui-capture-");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            builder.permissions(std::fs::Permissions::from_mode(0o700));
        }
        let temporary = builder.tempdir()?;
        let image = temporary.path().join("region.png");
        let rectangle = format!(
            "{},{},{},{}",
            region.x, region.y, region.width, region.height
        );
        let mut command = Command::new("/usr/bin/scrot");
        command.args(["-z", "-a", &rectangle]).arg(&image);
        run_bounded(command, cancel, None, Some((&image, MAX_ARTIFACT_BYTES)))?;
        if source_geometry(cancel)? != source {
            return Err(io::Error::other(
                "AcquisitionUnavailable: capture source geometry changed",
            ));
        }
        verify_window(request, cancel)?;
        let mut bytes = Vec::new();
        File::open(&image)?
            .take(MAX_ARTIFACT_BYTES + 1)
            .read_to_end(&mut bytes)?;
        validate_png(&bytes, region)?;
        if cancel.is_cancelled() {
            return Err(io::Error::other("acquisition cancelled"));
        }
        Ok(AcquiredVisual {
            bytes,
            region,
            quality: CaptureQuality::CompositedScreenSnapshot,
            capture_source_bytes: region.width as u64 * region.height as u64 * 4,
        })
    }
}

fn verify_window(request: SemanticVisualRegion, cancel: &CancellationToken) -> io::Result<()> {
    let tree = probe("/usr/bin/xwininfo", &["-root", "-tree"], cancel)?;
    let matches: Vec<_> = tree
        .lines()
        .filter_map(|line| {
            let words: Vec<_> = line.split_whitespace().collect();
            if words.len() < 3 || !words[0].starts_with("0x") {
                return None;
            }
            let geometry = words[words.len() - 2];
            let position = words[words.len() - 1].strip_prefix('+')?;
            let (x, y) = position.split_once('+')?;
            let (w, rest) = geometry.split_once('x')?;
            let h = rest.split('+').next()?;
            Some((
                words[0],
                ScreenRegion {
                    x: x.parse().ok()?,
                    y: y.parse().ok()?,
                    width: w.parse().ok()?,
                    height: h.parse().ok()?,
                },
            ))
        })
        .filter(|(_, bounds)| *bounds == request.window)
        .collect();
    let w = request.window;
    let r = request.bounds;
    if matches.len() != 1
        || r.x < w.x
        || r.y < w.y
        || i64::from(r.x) + i64::from(r.width) > i64::from(w.x) + i64::from(w.width)
        || i64::from(r.y) + i64::from(r.height) > i64::from(w.y) + i64::from(w.height)
    {
        return Err(io::Error::other(
            "AcquisitionUnavailable: AT-SPI bounds do not match a unique native client window",
        ));
    }
    let window_id = matches[0].0;
    if !window_id[2..].chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(io::Error::other(
            "AcquisitionUnavailable: invalid native window identifier",
        ));
    }
    let properties = probe(
        "/usr/bin/xprop",
        &["-id", window_id, "-notype", "_NET_WM_PID"],
        cancel,
    )?;
    let pid = properties
        .trim()
        .strip_prefix("_NET_WM_PID = ")
        .and_then(|s| s.parse::<u32>().ok());
    if pid != Some(request.process_id) {
        return Err(io::Error::other(
            "AcquisitionUnavailable: native window process does not match accessibility sender",
        ));
    }
    Ok(())
}

fn source_geometry(cancel: &CancellationToken) -> io::Result<(i32, i32)> {
    for key in [
        "GDK_SCALE",
        "GDK_DPI_SCALE",
        "QT_SCALE_FACTOR",
        "QT_SCREEN_SCALE_FACTORS",
    ] {
        if std::env::var(key).is_ok_and(|v| !v.is_empty() && v != "1") {
            return Err(io::Error::other(
                "AcquisitionUnavailable: explicit display scaling is unsupported",
            ));
        }
    }
    let info = probe("/usr/bin/xdpyinfo", &[], cancel)?;
    let monitors = probe("/usr/bin/xrandr", &["--listactivemonitors"], cancel)?;
    let transforms = probe("/usr/bin/xrandr", &["--verbose"], cancel)?;
    let xrdb = probe("/usr/bin/xrdb", &["-query"], cancel)?;
    parse_geometry(&info, &monitors, &transforms, &xrdb)
}

fn parse_geometry(
    info: &str,
    monitors: &str,
    transforms: &str,
    xrdb: &str,
) -> io::Result<(i32, i32)> {
    let unavailable = || {
        io::Error::other(
            "AcquisitionUnavailable: ambiguous screen, DPI, monitor origin, rotation or transform",
        )
    };
    if !info
        .lines()
        .any(|l| l.split_whitespace().collect::<Vec<_>>() == ["number", "of", "screens:", "1"])
        || !info.lines().any(|l| {
            l.split_whitespace().collect::<Vec<_>>()
                == ["resolution:", "96x96", "dots", "per", "inch"]
        })
        || monitors.lines().next() != Some("Monitors: 1")
        || !monitors
            .lines()
            .nth(1)
            .is_some_and(|l| l.split_whitespace().any(|s| s.ends_with("+0+0")))
        || xrdb
            .lines()
            .any(|l| l.trim().starts_with("Xft.dpi:") && l.split_whitespace().last() != Some("96"))
    {
        return Err(unavailable());
    }
    let lines: Vec<_> = transforms.lines().map(str::trim).collect();
    let positions: Vec<_> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("Transform:"))
        .map(|(i, _)| i)
        .collect();
    if positions.len() != 1 {
        return Err(unavailable());
    }
    let p = positions[0];
    if lines[p].split_whitespace().collect::<Vec<_>>()
        != ["Transform:", "1.000000", "0.000000", "0.000000"]
        || lines
            .get(p + 1)
            .map(|s| s.split_whitespace().collect::<Vec<_>>())
            != Some(vec!["0.000000", "1.000000", "0.000000"])
        || lines
            .get(p + 2)
            .map(|s| s.split_whitespace().collect::<Vec<_>>())
            != Some(vec!["0.000000", "0.000000", "1.000000"])
        || lines.iter().filter(|l| l.contains(" connected ")).any(|l| {
            l.split('(').next().is_some_and(|h| {
                h.split_whitespace()
                    .any(|s| ["left", "right", "inverted"].contains(&s))
            })
        })
    {
        return Err(unavailable());
    }
    let dimensions = info
        .lines()
        .find_map(|l| l.trim().strip_prefix("dimensions:"))
        .and_then(|l| l.split_whitespace().next())
        .ok_or_else(unavailable)?;
    let (w, h) = dimensions.split_once('x').ok_or_else(unavailable)?;
    Ok((
        w.parse().map_err(|_| unavailable())?,
        h.parse().map_err(|_| unavailable())?,
    ))
}

fn probe(program: &str, args: &[&str], cancel: &CancellationToken) -> io::Result<String> {
    let output = tempfile::tempfile()?;
    let mut command = Command::new(program);
    command.args(args).env("LC_ALL", "C");
    run_bounded(command, cancel, Some(output.try_clone()?), None)?;
    use std::io::{Seek, SeekFrom};
    let mut output = output;
    output.seek(SeekFrom::Start(0))?;
    let mut text = String::new();
    output.take(65537).read_to_string(&mut text)?;
    if text.len() > 65536 {
        return Err(io::Error::other("capture environment probe exceeds limit"));
    }
    Ok(text)
}

fn run_bounded(
    mut command: Command,
    cancel: &CancellationToken,
    stdout: Option<File>,
    artifact: Option<(&std::path::Path, u64)>,
) -> io::Result<()> {
    command.stdin(Stdio::null()).stderr(Stdio::null());
    let monitor = stdout.as_ref().map(File::try_clone).transpose()?;
    command.stdout(stdout.map(Stdio::from).unwrap_or_else(Stdio::null));
    let mut child = command.spawn()?;
    let start = Instant::now();
    loop {
        let oversized = monitor
            .as_ref()
            .is_some_and(|f| f.metadata().is_ok_and(|m| m.len() > 65536))
            || artifact.is_some_and(|(path, max)| path.metadata().is_ok_and(|m| m.len() > max));
        if cancel.is_cancelled() || start.elapsed() > Duration::from_secs(5) || oversized {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other(
                "capture cancelled, timed out or exceeded size limit",
            ));
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err(io::Error::other(
                        "static capture environment command failed",
                    ))
                };
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        }
    }
}

fn validate_png(bytes: &[u8], region: ScreenRegion) -> io::Result<()> {
    if bytes.len() < 24
        || bytes.len() as u64 > MAX_ARTIFACT_BYTES
        || &bytes[..8] != b"\x89PNG\r\n\x1a\n"
        || &bytes[12..16] != b"IHDR"
        || bytes[16..20] != (region.width as u32).to_be_bytes()
        || bytes[20..24] != (region.height as u32).to_be_bytes()
    {
        return Err(io::Error::other(
            "AcquisitionUnavailable: capture is not a bounded exact-region PNG",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_mapping_rejects_scaled_or_multiple_monitors() {
        let info =
            "number of screens: 1\nresolution: 96x96 dots per inch\ndimensions: 1280x800 pixels";
        let monitor = "Monitors: 1\n 0: +screen 1280/339x800/212+0+0 screen";
        let transform = "Transform: 1.000000 0.000000 0.000000\n0.000000 1.000000 0.000000\n0.000000 0.000000 1.000000";
        assert_eq!(
            parse_geometry(info, monitor, transform, "").unwrap(),
            (1280, 800)
        );
        assert!(parse_geometry(info, "Monitors: 2", transform, "").is_err());
        assert!(
            parse_geometry(
                info,
                monitor,
                &transform.replace("1.000000", "2.000000"),
                ""
            )
            .is_err()
        );
        assert!(parse_geometry(&info.replace("96x96", "192x192"), monitor, transform, "").is_err());
        assert!(parse_geometry(info, monitor, transform, "Xft.dpi: 144").is_err());
    }
}
