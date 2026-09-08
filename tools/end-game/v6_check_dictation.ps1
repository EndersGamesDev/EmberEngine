param([string]$Assets = (Join-Path $PSScriptRoot '../../assets/end-game/v6'))
# Optional free-dictation diagnostic, never microphone input or speaker playback.
$ErrorActionPreference = 'Stop'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass = 'Idle'
Add-Type -AssemblyName System.Speech
$recognizer = [System.Speech.Recognition.SpeechRecognitionEngine]::InstalledRecognizers() |
    Where-Object { $_.Culture.Name -eq 'en-GB' } | Select-Object -First 1
if ($null -eq $recognizer) { throw 'No installed en-GB dictation recognizer; no model is downloaded.' }
$timer = [Diagnostics.Stopwatch]::StartNew()
$rows = foreach ($clip in @('movement', 'unlocking', 'sword', 'death')) {
    $engine = [System.Speech.Recognition.SpeechRecognitionEngine]::new($recognizer.Id)
    try {
        $engine.LoadGrammar([System.Speech.Recognition.DictationGrammar]::new())
        $engine.SetInputToWaveFile((Join-Path $Assets ('warden-' + $clip + '.wav')))
        $result = $engine.Recognize()
        [PSCustomObject]@{ clip = $clip; text = $result.Text; confidence = $result.Confidence }
    } finally { $engine.Dispose() }
}
$timer.Stop()
[PSCustomObject]@{ recognizer = $recognizer.Id; files = $rows; seconds = $timer.Elapsed.TotalSeconds;
    scope = 'Optional free dictation, not human listening or exact-transcript validation; invented groans and expressive voice processing can be misrecognized.' } |
    ConvertTo-Json -Depth 5
