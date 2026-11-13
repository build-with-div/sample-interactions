# Drive the native probe on Windows: start it, post a scripted sequence of
# mouse messages to its window (the real cursor is not moved), close it, and
# print what it logged. Run from a shell with the build environment, after
# `cargo build --bin native_probe`.
#
#   pwsh tools/native-input.ps1 [-Exe <path to native_probe.exe>]
#
# Posted messages go through winit's window procedure exactly as device
# input does, but they are not device input: there is no OS mouse capture,
# no CS_DBLCLKS translation, no pen or touch. Coordinates are client pixels.
param(
    [string]$Exe = "$env:CARGO_TARGET_DIR\debug\native_probe.exe",
    [string]$Log = "$PSScriptRoot\..\out\native-probe.log"
)

Add-Type -Namespace Probe -Name User32 -MemberDefinition @'
[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindowW(string cls, string title);
[DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr hwnd, uint msg, IntPtr wparam, IntPtr lparam);
[DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hwnd, ref POINT point);
[DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hwnd);
[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr hwnd, System.Text.StringBuilder text, int max);
public delegate bool EnumProc(IntPtr hwnd, IntPtr data);
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc proc, IntPtr data);
public static IntPtr VisibleWindowOf(uint pid) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, d) => {
        uint owner; GetWindowThreadProcessId(h, out owner);
        if (owner == pid && IsWindowVisible(h)) { found = h; return false; }
        return true;
    }, IntPtr.Zero);
    return found;
}
public static string TitleOf(IntPtr hwnd) {
    var text = new System.Text.StringBuilder(256); GetWindowTextW(hwnd, text, 256); return text.ToString();
}
[StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
'@

$process = Start-Process -FilePath $Exe -RedirectStandardOutput $Log -PassThru -WindowStyle Normal
try {
    $hwnd = [IntPtr]::Zero
    for ($i = 0; $i -lt 100 -and $hwnd -eq [IntPtr]::Zero; $i++) {
        Start-Sleep -Milliseconds 100
        $ready = (Test-Path $Log) -and (Select-String -Path $Log -Pattern 'PLATFORM resized' -Quiet)
        if ($ready) { $hwnd = [Probe.User32]::VisibleWindowOf([uint32]$process.Id) }
    }
    "window '$([Probe.User32]::TitleOf($hwnd))'" | Write-Output
    if ($hwnd -eq [IntPtr]::Zero) { throw 'the probe window did not appear' }
    Start-Sleep -Milliseconds 500
    $scale = [Probe.User32]::GetDpiForWindow($hwnd) / 96.0
    "DPI scale $scale" | Write-Output

    function LParam([double]$x, [double]$y) {
        # Logical CSS px to physical client px, packed as two signed 16-bit words.
        $px = [int][Math]::Round($x * $scale) -band 0xFFFF
        $py = [int][Math]::Round($y * $scale) -band 0xFFFF
        [IntPtr](($py -shl 16) -bor $px)
    }
    $script:seen = 0
    function Post([uint32]$msg, [int64]$wparam, [IntPtr]$lparam, [string]$note) {
        if ($note) { "STEP $note" | Write-Output }
        [void][Probe.User32]::PostMessageW($hwnd, $msg, [IntPtr]$wparam, $lparam)
        Start-Sleep -Milliseconds 80
        # Show what the probe logged for this step, under it.
        $lines = @(Get-Content $Log -ErrorAction SilentlyContinue)
        $lines | Select-Object -Skip $script:seen | ForEach-Object { "    $_" | Write-Output }
        $script:seen = $lines.Count
    }
    $MOVE = 0x200; $LDOWN = 0x201; $LUP = 0x202; $RDOWN = 0x204; $RUP = 0x205
    $MDOWN = 0x207; $MUP = 0x208; $WHEEL = 0x20A; $HWHEEL = 0x20E; $CLOSE = 0x10
    $MK_LBUTTON = 1

    Post $MOVE 0 (LParam 100 100) 'move over the handle'
    Post $LDOWN $MK_LBUTTON (LParam 100 100) 'press on the handle'
    Post $MOVE $MK_LBUTTON (LParam 350 100) 'drag over #other'
    Post $MOVE $MK_LBUTTON (LParam -30 100) 'drag left of the window (negative client x)'
    Post $LUP 0 (LParam -30 100) 'release outside the window'
    Post $MOVE 0 (LParam 350 100) 'hover #other after the release'
    Post $RDOWN 2 (LParam 350 100) 'right press on #other'
    Post $RUP 0 (LParam 350 100) 'right release'
    Post $MDOWN 0x10 (LParam 350 100) 'middle press'
    Post $MUP 0 (LParam 350 100) 'middle release'
    Post $LDOWN $MK_LBUTTON (LParam 350 100) 'double click: first press'
    Post $LUP 0 (LParam 350 100) ''
    Post $LDOWN $MK_LBUTTON (LParam 350 100) 'double click: second press'
    Post $LUP 0 (LParam 350 100) ''
    # Wheel messages carry screen coordinates.
    $point = New-Object Probe.User32+POINT
    $point.X = [int](400 * $scale); $point.Y = [int](275 * $scale)
    [void][Probe.User32]::ClientToScreen($hwnd, [ref]$point)
    $screen = [IntPtr]((($point.Y -band 0xFFFF) -shl 16) -bor ($point.X -band 0xFFFF))
    Post $MOVE 0 (LParam 400 275) 'move over the scroller'
    Post $WHEEL ([int64](120 -shl 16)) $screen 'wheel one notch forward (away from the user: scroll up in every Windows app)'
    Post $WHEEL ([int64](120 -shl 16)) $screen 'again'
    Post $HWHEEL ([int64](120 -shl 16)) $screen 'horizontal wheel one notch right'
    Start-Sleep -Milliseconds 300
    Post $CLOSE 0 ([IntPtr]::Zero) 'close'
    if (-not $process.WaitForExit(5000)) { $process.Kill() }
} finally {
    if (-not $process.HasExited) { $process.Kill() }
}
Get-Content $Log | Select-Object -Skip $script:seen | ForEach-Object { "    $_" | Write-Output }
