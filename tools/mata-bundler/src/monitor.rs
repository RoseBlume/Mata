//! Dependency-free process monitoring and SVG reports.
//!
//! Accurate, unprivileged FPS comes from an in-process frame counter supplied
//! by the monitored app through MATA_MONITOR_FPS_FILE. There is no automatic
//! privileged ETW trace, PresentMon download, or administrator request.
//! CPU/RSS belong to the app; network is HOST traffic if per-application byte
//! counters are unavailable.
use std::{collections::BTreeMap, fs::{self, File}, io::{self, BufRead, Write}, path::Path, process::{Command, Stdio}, sync::mpsc, thread, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};
use crate::{bail, cli::{Options, Platform}, config, desktop, project::Project, tools, Result};

const METRICS: [(&str, &str, &str); 6] = [
    ("fps", "Frames per second", "FPS"),
    ("gpu", "Process GPU engine usage", "%"),
    ("cpu", "Process CPU usage (normalized to all logical cores)", "%"),
    ("memory", "Process resident memory", "MiB"),
    ("network_rx", "Network received (application telemetry or system-wide fallback)", "KiB/s"),
    ("network_tx", "Network sent (application telemetry or system-wide fallback)", "KiB/s"),
];
#[derive(Clone, Copy, Debug)]
struct Point { sec: f64, val: f64 }
#[derive(Default)]
struct Series { points: Vec<Point> }
impl Series {
    fn change(&mut self, sec: f64, val: Option<f64>, digits: f64) {
        let Some(value) = val.filter(|v| v.is_finite() && *v >= 0.0) else { return };
        let rounded = (value * digits).round() / digits;
        if self.points.last().is_none_or(|p| (p.val - rounded).abs() > 0.000001) {
            self.points.push(Point { sec, val: rounded });
        }
    }
}
struct Data { series: BTreeMap<&'static str, Series>, samples: usize, elapsed: f64, network_process: bool, warnings: Vec<String> }
impl Data {
    fn new() -> Self { Self { series: METRICS.iter().map(|m| (m.0,Series::default())).collect(), samples: 0, elapsed: 0.0, network_process: false, warnings: Vec::new() } }
    fn push(&mut self, key: &'static str, sec: f64, value: Option<f64>, digits: f64) {
        if let Some(s) = self.series.get_mut(key) { s.change(sec,value,digits); }
    }
}

pub fn run(platform: Platform, opts: &Options) -> Result<()> {
    if platform != Platform::host()? { return bail("mata monitor must run a native executable for this host; cross-target monitoring is not supported"); }
    let root = tools::project_root(opts.project_dir.as_deref())?;
    let cfg = config::load(&root)?;
    let project = Project::discover(&root, &cfg, opts)?;
    // `mata monitor` is the ONLY Mata command that opts into MataWin
    // instrumentation. `mata build` and `mata dev` stay unchanged.
    let mut monitored_opts = opts.clone();
    if platform == Platform::Windows {
        if let Some(alias) = &project.matawin_dependency {
            monitored_opts.features.push(format!("{alias}/telemetry"));
            println!("Windows MataWin FPS telemetry: enabling {alias}/telemetry for this monitor build");
        } else {
            eprintln!("FPS note: selected package has no direct mata_win dependency; monitoring will run without automatic MataWin FPS. If your renderer uses MataWin indirectly, add it as a direct dependency or forward its telemetry feature.");
        }
    }
    let binary = desktop::build(&project, &cfg, platform, &monitored_opts)?;
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let directory = opts.output.as_ref().map(|p| tools::resolve_output(&root,p))
        .unwrap_or_else(|| project.target_dir.join("mata/monitor"))
        .join(format!("{}-{}",project.package,stamp));
    fs::create_dir_all(&directory)?;
    let fps_path = opts.fps_file.as_ref().map(|p| tools::resolve_output(&root,p)).unwrap_or_else(|| directory.join("app-fps.txt"));
    if opts.fps_file.is_none() { let _ = fs::remove_file(&fps_path); }
    let network_path = directory.join("app-network.txt");
    // This telemetry file is created only by a cooperating application. No
    // unprivileged Windows API exposes exact presented-frame counts for arbitrary
    // other processes, so never substitute compositor or CPU/GPU rate estimates.
    let mut app = Command::new(&binary)
        .current_dir(&project.package_root)
        .env("MATA_MONITOR_FPS_FILE",&fps_path)
        .env("MATA_MONITOR_NETWORK_FILE",&network_path)
        .stdin(Stdio::null())
        .spawn()?;
    let pid = app.id();
    println!("FPS: waiting for application frame telemetry at {}", fps_path.display());
    if platform == Platform::Windows {
        println!("FPS: MataWin will publish presented-frame counts automatically when its telemetry feature is active.");
    } else {
        println!("FPS: this Linux application must supply frame telemetry (see integrations/mata_fps.rs).");
    }
    crate::interrupt::install()?;
    let mut data = Data::new();
    let mut sampler = ProcessSampler::new(platform, pid);
    let (exit_tx, exit_rx) = mpsc::channel::<()>();
    // Keep console input under control of monitor; app stdin is disconnected.
    thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(line) if line.trim().eq_ignore_ascii_case("exit") => { let _ = exit_tx.send(()); break; }
                Ok(_) => eprintln!("mata monitor: type 'exit' to stop monitoring"),
                Err(_) => break,
            }
        }
    });
    let interval = Duration::from_millis(opts.poll_ms);
    let started = Instant::now();
    let mut stopped = false;
    println!("Mata monitor: PID {pid} | {} | every {} ms", if opts.release { "release" } else { "debug" }, opts.poll_ms);
    println!("Type 'exit' + Enter (or Ctrl+C) to stop and generate the report.");
    println!("Report directory: {}", directory.display());
    let mut last_present = Instant::now();
    loop {
        if let Some(status) = app.try_wait()? { println!("Program exited: {status}"); break; }
        if exit_rx.try_recv().is_ok() || crate::interrupt::requested() {
            stopped = true;
            println!("Stopping monitored application...");
            break;
        }
        let sec = started.elapsed().as_secs_f64();
        sampler.collect(sec, &fps_path, &network_path, &mut data);
        data.samples += 1;
        data.elapsed = sec;
        if last_present.elapsed() >= Duration::from_secs(3) {
            print_live(&data);
            last_present = Instant::now();
        }
        // Sleep in short slices so 'exit' and process termination are responsive.
        let until = Instant::now() + interval;
        while Instant::now() < until {
            if exit_rx.try_recv().is_ok() || crate::interrupt::requested() { stopped = true; break; }
            if app.try_wait()?.is_some() { break; }
            thread::sleep(Duration::from_millis(75).min(until.saturating_duration_since(Instant::now())));
        }
        if stopped { break; }
    }
    if stopped { let _ = app.kill(); }
    let _ = app.wait();
    data.elapsed = started.elapsed().as_secs_f64();
    if data.series["fps"].points.is_empty() {
        data.warnings.push(format!("FPS unavailable: the application did not report completed frames to {}. On Windows, confirm the app presents through mata_win and depends on mata_win with the 'telemetry' feature available; other rendering backends need their own frame hook. No elevated tracing was attempted.",fps_path.display()));
    }
    write_report(&directory, &data, &project, pid, opts)?;
    println!("Monitor report: {}", directory.join("index.html").display());
    println!("Changes recorded: {}",data.series.values().map(|s|s.points.len()).sum::<usize>());
    Ok(())
}
fn print_live(data: &Data) {
    let entry = |key: &str| data.series.get(key).and_then(|s| s.points.last()).map(|p| format!("{:.1}", p.val)).unwrap_or_else(|| "N/A".into());
    println!("[{:.0}s] FPS {} | CPU {}% | GPU {}% | RSS {} MiB | Network ↓{} ↑{} KiB/s",data.elapsed,entry("fps"),entry("cpu"),entry("gpu"),entry("memory"),entry("network_rx"),entry("network_tx"));
}

struct ProcessSampler {
    platform: Platform, pid: u32,
    prev_cpu: Option<(f64, Instant)>,
    prev_gpu: Option<(f64, Instant)>,
    prev_net: Option<((u64,u64), Instant)>,
    cpus: f64,
    clock_ticks: f64,
}
impl ProcessSampler {
    fn new(platform: Platform, pid: u32) -> Self {
        Self { platform, pid, prev_cpu: None, prev_gpu: None, prev_net: None,
            cpus: thread::available_parallelism().map(|n|n.get()).unwrap_or(1) as f64,
            clock_ticks: getconf("CLK_TCK").unwrap_or(100) as f64 }
    }
    fn collect(&mut self, sec: f64, fps_path: &Path, network_path: &Path, data: &mut Data) {
        let fps = reported_fps(fps_path);
        data.push("fps",sec,fps,100.0);
        let (cpu_seconds, memory_bytes, gpu_seconds, host_net) = match self.platform {
            Platform::Linux => {
                let (ticks, rss) = linux_proc(self.pid).unwrap_or_default();
                (ticks.map(|v|v as f64 / self.clock_ticks),rss,linux_gpu(self.pid).map(|ns|ns as f64/1e9),linux_net())
            }
            Platform::Windows => {
                let (cpu,rss,gpu,net) = windows_proc(self.pid);
                (cpu,rss,gpu,net)
            }
            Platform::Android => (None,None,None,None),
        };
        let current = Instant::now();
        let cpu = match (cpu_seconds,self.prev_cpu) {
            (Some(value),Some((old,at))) if value >= old => Some(((value-old)/current.duration_since(at).as_secs_f64()/self.cpus*100.0).clamp(0.0,100.0)),
            _ => None,
        };
        if let Some(v) = cpu_seconds { self.prev_cpu = Some((v,current)); }
        data.push("cpu", sec, cpu, 10.0);
        data.push("memory", sec, memory_bytes.map(|m|m as f64/(1024.0*1024.0)), 10.0);
        // Linux DRM fdinfo counters are cumulative nanoseconds; Windows GPU
        // Engine performance counters return instantaneous engine utilization.
        let gpu = if self.platform == Platform::Linux {
            let value = match (gpu_seconds,self.prev_gpu) {
                (Some(v),Some((old,at))) if v >= old => Some(((v-old)/current.duration_since(at).as_secs_f64()*100.0).clamp(0.0,100.0)),
                _ => None,
            };
            if let Some(v) = gpu_seconds { self.prev_gpu = Some((v,current)); }
            value
        } else { gpu_seconds };
        data.push("gpu",sec,gpu,10.0);
        let app_net = fs::read_to_string(network_path).ok().and_then(|s| {
            let mut bits=s.split_whitespace();
            Some((bits.next()?.parse::<u64>().ok()?, bits.next()?.parse::<u64>().ok()?))
        });
        if app_net.is_some() && !data.network_process {
            // Never combine system-level traffic with app-specific telemetry.
            data.network_process=true;
            data.series.get_mut("network_rx").unwrap().points.clear();
            data.series.get_mut("network_tx").unwrap().points.clear();
            self.prev_net=None;
        }
        let chosen_net = if data.network_process {app_net} else {host_net};
        if let Some((rx,tx)) = chosen_net {
            if let Some(((prx,ptx),at)) = self.prev_net {
                let dt = current.duration_since(at).as_secs_f64();
                if dt>0.0 { data.push("network_rx",sec,rx.checked_sub(prx).map(|n|n as f64 /dt/1024.0),10.0);
                    data.push("network_tx",sec,tx.checked_sub(ptx).map(|n|n as f64 /dt/1024.0),10.0); }
            }
            self.prev_net = Some(((rx,tx),current));
        }
    }
}
fn getconf(key: &str) -> Option<u64> {
    let result=Command::new("getconf").arg(key).output().ok()?;
    if !result.status.success() {return None;}
    String::from_utf8(result.stdout).ok()?.trim().parse().ok()
}
fn linux_proc(pid: u32) -> Option<(Option<u64>,Option<u64>)> {
    let content=fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let end=content.rfind(')')?;
    let fields:Vec<_>=content[end+1..].split_whitespace().collect();
    let ticks: u64=fields.get(11)?.parse::<u64>().ok()?.saturating_add(fields.get(12)?.parse::<u64>().ok()?);
    let status=fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let mem=status.lines().find_map(|l|l.strip_prefix("VmRSS:").and_then(|v|v.split_whitespace().next()).and_then(|v|v.parse::<u64>().ok()).map(|kb|kb*1024));
    Some((Some(ticks),mem))
}
fn linux_net() -> Option<(u64,u64)> {
    let text=fs::read_to_string("/proc/net/dev").ok()?;
    let mut rx=0u64; let mut tx=0u64;
    for row in text.lines().skip(2) {
        let (name,values)=match row.split_once(':'){Some(v)=>v,None=>continue};
        if name.trim()=="lo" {continue;}
        let fields:Vec<_>=values.split_whitespace().collect();
        rx=rx.saturating_add(fields.first()?.parse::<u64>().ok()?);
        tx=tx.saturating_add(fields.get(8)?.parse::<u64>().ok()?);
    }
    Some((rx,tx))
}
fn linux_gpu(pid: u32) -> Option<u64> {
    let path=format!("/proc/{pid}/fdinfo");
    let mut clients=BTreeMap::<String,BTreeMap<String,u64>>::new();
    for entry in fs::read_dir(path).ok()?.flatten() {
        let Ok(text)=fs::read_to_string(entry.path()) else { continue };
        let mut client=None;
        let mut engines=BTreeMap::<String,u64>::new();
        for line in text.lines() {
            if let Some(v)=line.strip_prefix("drm-client-id:") {client=Some(v.trim().to_string());}
            else if let Some((kind,value))=line.split_once(':') {
                if kind.starts_with("drm-engine-") {
                    if let Some(ns)=value.split_whitespace().next().and_then(|v|v.parse::<u64>().ok()) {engines.insert(kind.into(),ns);}
                }
            }
        }
        if !engines.is_empty() {
            // Multiple descriptors can refer to the same DRM client.
            let name=client.unwrap_or_else(|| entry.path().display().to_string());
            clients.entry(name).or_insert(engines);
        }
    }
    if clients.is_empty() {return None;}
    Some(clients.values().flat_map(|v|v.values()).copied().sum())
}
fn windows_proc(pid: u32) -> (Option<f64>,Option<u64>,Option<f64>,Option<(u64,u64)>) {
    // One bounded PowerShell subprocess; PID-based, never a process-name guess.
    // GPU counters may be unavailable depending on driver/Windows configuration.
    let script=format!(r#"$ErrorActionPreference='SilentlyContinue';
$p=Get-Process -Id {pid} -ErrorAction Stop;
$gpu='NA';try{{$s=Get-Counter '\GPU Engine(*)\Utilization Percentage' -ErrorAction Stop;$v=($s.CounterSamples|Where-Object {{$_.Path -match 'pid_{pid}_'}}|Measure-Object -Property CookedValue -Sum).Sum;if($null -ne $v){{$gpu=[string][double]$v}}}}catch{{}};
$rx=0;$tx=0;try{{$n=Get-NetAdapterStatistics -ErrorAction Stop;$rx=($n|Measure-Object -Property ReceivedBytes -Sum).Sum;$tx=($n|Measure-Object -Property SentBytes -Sum).Sum}}catch{{}};
Write-Output (([double]$p.CPU).ToString([Globalization.CultureInfo]::InvariantCulture)+'|'+$p.WorkingSet64.ToString()+'|'+$gpu+'|'+$rx+'|'+$tx)"#);
    let Ok(output)=Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",&script]).output() else {return (None,None,None,None)};
    if !output.status.success() {return (None,None,None,None)}
    let text=String::from_utf8_lossy(&output.stdout);
    let fields:Vec<_>=text.lines().last().unwrap_or("").trim().split('|').collect();
    let num=|i:usize|fields.get(i).and_then(|s|s.trim().parse::<f64>().ok());
    let bytes=|i:usize|fields.get(i).and_then(|s|s.trim().parse::<u64>().ok());
    (num(0),bytes(1),num(2),bytes(3).zip(bytes(4)))
}

/// Read a freshly reported in-application frame rate. If the application has
/// stopped presenting, show 0 instead of incorrectly retaining a stale FPS.
/// A missing/unreadable telemetry file means FPS is unknown, not zero.
fn reported_fps(path: &Path) -> Option<f64> {
    let text = fs::read_to_string(path).ok()?;
    let value = text.trim().parse::<f64>().ok()?;
    if !value.is_finite() || !(0.0..=100_000.0).contains(&value) { return None; }
    let age = SystemTime::now().duration_since(fs::metadata(path).ok()?.modified().ok()?)
        .unwrap_or(Duration::ZERO);
    if age > Duration::from_secs(2) { Some(0.0) } else { Some(value) }
}

fn xml(s:&str)->String {
    s.replace('&',"&amp;").replace('<',"&lt;").replace('>',"&gt;").replace('"',"&quot;").replace('\'',"&apos;")
}
fn graph_svg(path:&Path,caption:&str,unit:&str,series:&Series,elapsed:f64)->Result<()> {
    let width=1280.0;let height=480.0;
    let l=90.0;let t=82.0;let r=1240.0;let b=408.0;
    let mut svg=format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width} {height}\"><rect width=\"100%\" height=\"100%\" fill=\"#101827\"/><style>text{{font:15px system-ui,sans-serif;fill:#dbe6fb}}.grid{{stroke:#33445b;stroke-width:1;stroke-dasharray:5 5}}</style><text x=\"36\" y=\"45\" style=\"font-size:23px;font-weight:600\">{}</text><text x=\"36\" y=\"69\" style=\"font-size:13px\">Each point is stored only when the measured value changes. Missing samples are not shown as zero.</text>\n",xml(caption));
    if series.points.is_empty() {
        svg.push_str("<text x=\"130\" y=\"246\" style=\"font-size:22px\">No available measurements for this metric.</text></svg>");
        fs::write(path,svg)?;return Ok(());
    }
    let max_time=elapsed.max(series.points.last().map(|p|p.sec).unwrap_or(0.0)).max(1.0);
    let high=series.points.iter().fold(0.0f64,|max,p|max.max(p.val));
    let upper=if high<0.001 {1.0}else{(high*1.13).max(0.1)};
    for i in 0..=5 {
        let y=b-(b-t)*i as f64/5.0;
        let value=upper*i as f64/5.0;
        svg.push_str(&format!("<line class=\"grid\" x1=\"{l}\" y1=\"{y:.1}\" x2=\"{r}\" y2=\"{y:.1}\"/><text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\">{:.1}</text>\n",l-10.0,y+5.0,value));
    }
    for i in 0..=6 {
        let x=l+(r-l)*i as f64/6.0;
        svg.push_str(&format!("<line class=\"grid\" x1=\"{x:.1}\" y1=\"{t}\" x2=\"{x:.1}\" y2=\"{b}\"/><text x=\"{x:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{:.0}s</text>\n",b+26.0,max_time*i as f64/6.0));
    }
    let x_at=|s:f64|l+(r-l)*(s/max_time).clamp(0.0,1.0);
    let y_at=|v:f64|b-(b-t)*(v/upper).clamp(0.0,1.0);
    let mut path_d=String::new();
    for (i,p) in series.points.iter().enumerate() {
        let x=x_at(p.sec);let y=y_at(p.val);
        if i==0 {path_d.push_str(&format!("M{x:.2},{y:.2}"));}
        else {path_d.push_str(&format!(" H{x:.2} V{y:.2}"));}
    }
    let last=series.points.last().unwrap();
    path_d.push_str(&format!(" H{:.2}",x_at(max_time)));
    svg.push_str(&format!("<path d=\"{path_d}\" fill=\"none\" stroke=\"#65d4ec\" stroke-width=\"3\" stroke-linejoin=\"round\"/><text x=\"{l}\" y=\"{:.1}\">{} | {} changes | last {:.2} {}</text></svg>\n",height-17.0,xml(unit),series.points.len(),last.val,xml(unit)));
    fs::write(path,svg)?;Ok(())
}
fn write_report(dir:&Path,data:&Data,project:&Project,pid:u32,opts:&Options)->Result<()> {
    let mut changes=Vec::new();
    for (name,description,unit) in METRICS {
        let series=&data.series[name];
        graph_svg(&dir.join(format!("{name}.svg")),description,unit,series,data.elapsed)?;
        let mut csv=File::create(dir.join(format!("{name}.csv")))?;
        writeln!(csv,"elapsed_seconds,value,unit")?;
        for p in &series.points {
            writeln!(csv,"{:.6},{:.4},{unit}",p.sec,p.val)?;
            changes.push((p.sec,name,p.val,unit));
        }
    }
    changes.sort_by(|a,b|a.0.total_cmp(&b.0).then(a.1.cmp(b.1)));
    let mut changes_csv=File::create(dir.join("changes.csv"))?;
    writeln!(changes_csv,"elapsed_seconds,metric,value,unit")?;
    for (sec,name,val,unit) in changes {writeln!(changes_csv,"{sec:.6},{name},{val:.4},{unit}")?;}
    let mut html=format!(r#"<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Mata monitor: {}</title>
<style>body{{font:16px system-ui,sans-serif;max-width:1250px;margin:2rem auto;padding:0 1rem;background:#101827;color:#e6efff}}a{{color:#74d8ff}}img{{max-width:100%;border:1px solid #33445b;border-radius:9px}}article{{margin:2rem 0}}.warning{{background:#27222a;padding:8px 15px;border-radius:8px}}small{{color:#9eafc5}}</style>
<h1>Mata monitor — {}</h1><p>PID: {} | Profile: {} | Duration: {:.2}s | Sampling passes: {} | Interval: {}ms</p><p><a href="changes.csv">All measured changes (CSV)</a></p>
<p><small>CPU is normalized across logical processors. GPU is measured only where OS/driver counters are available. Network defaults to SYSTEM-WIDE unless app telemetry is enabled. FPS is reported by the application's rendering loop (no administrator privileges or ETW required). Blank metrics are unavailable, not zero.</small></p>"#,xml(&project.name),xml(&project.name),pid,if opts.release{"release"}else{"debug"},data.elapsed,data.samples,opts.poll_ms);
    html.push_str(&format!("<p><strong>Network measurement scope:</strong> {}.</p>",if data.network_process {"Monitored application (cumulative byte counters supplied by the app)"} else {"Entire host (not process-specific)"}));
    for (key,label,unit) in METRICS {
        html.push_str(&format!("<article><h2>{}</h2><img src=\"{}.svg\" alt=\"{} graph\"><p><a href=\"{}.csv\">Raw change points ({})</a></p></article>",xml(label),key,xml(label),key,xml(unit)));
    }
    if !data.warnings.is_empty() {
        html.push_str("<section class=\"warning\"><h2>Measurement notes</h2><ul>");
        for warning in &data.warnings {html.push_str(&format!("<li>{}</li>",xml(warning)));}
        html.push_str("</ul></section>");
    }
    html.push_str("</html>\n");
    fs::write(dir.join("index.html"),html)?;
    Ok(())
}
#[cfg(test)]mod tests {
    use super::*;
    #[test] fn changes_only() {
        let mut s=Series::default();s.change(0.0,Some(2.11),10.0);s.change(0.5,Some(2.12),10.0);s.change(1.5,Some(2.2),10.0);
        assert_eq!(s.points.len(),2);
    }
    #[test] fn no_fake_values() {
        let mut s=Series::default();s.change(0.0,None,10.0);s.change(0.1,Some(f64::NAN),10.0);
        assert!(s.points.is_empty());
    }
    #[test] fn telemetry_reads_real_values_and_rejects_invalid() {
        let path=std::env::temp_dir().join(format!("mata-fps-test-{}-{}.txt", std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        assert_eq!(reported_fps(&path), None);
        fs::write(&path, "72.5\n").unwrap();
        assert_eq!(reported_fps(&path), Some(72.5));
        fs::write(&path, "NaN").unwrap();
        assert_eq!(reported_fps(&path), None);
        let _=fs::remove_file(path);
    }

}
