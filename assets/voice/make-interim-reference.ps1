# Creates the INTERIM ION reference clip: Microsoft David (local SAPI voice)
# reading original text. Clearly a stopgap until a refined British male
# reference is supplied. Dry/24kHz for cloning quality.
Add-Type -AssemblyName System.Speech
$synth = New-Object System.Speech.Synthesis.SpeechSynthesizer
$synth.SelectVoice("Microsoft David Desktop")
$synth.Rate = -2
$synth.SetOutputToWaveFile("$PSScriptRoot\ion-reference.wav",
  [System.Speech.AudioFormat.SpeechAudioFormatInfo]::new(24000, [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen, [System.Speech.AudioFormat.AudioChannel]::Mono))
$text = @"
All systems are nominal. Power levels remain stable.
Ion Sense is monitoring your battery, temperatures, and downloads.
Battery level is critically low. Please connect a power source.
Thermal activity has reached a critical level.
Your download has completed. A new message has arrived.
I will surface whatever needs your attention.
"@
$synth.Speak($text)
$synth.Dispose()
"reference written: $PSScriptRoot\ion-reference.wav"
