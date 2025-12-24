The app is a speech-to-text TUI + CLI written in Rust using Ratatui. 

## Jargon

Archivum = Database
Codex = Project
Folio = Recording
Fragmentum = Chunk transcription from the recording

## Functions

### Audio recording function

We need to implement a function that records the audio from the microphone in pre-determined chunks of audio (i.e. every e.g. 10 s it creates an wav file).
This has to endup in src/stt/audio.rs. The length of the chunks should be free at the moment (use it as a param to the function). 
It must record in mono at a sample rate of 16kHz. Use the utilities in src/stt/audio.rs if the audio is not natively in 16kHz. 

### Continuous transcription function

There should be a function that when triggered, records the audio and at the end of the pre-determined chunk transcribes it a then 
a) stores the wav file in the data directory .local/share/scriptor
b) adds a new fragmentum to the DB.
Notice that the folio is the uninterrupted recording and fragmenta are the chunks. So if I record for 30s and get 3 fragmenta, I still have 1 folio. 
Notice also that this function should update the DB also with the path to the audio file that was saved

## Temp

momentarily the model is stored in models/ but really it should end up in the data diretory, likely something like
~/local/share/scriptor/models/parakeet. Copy it there for the time being we'll then need to figure out a way to store the model. 

## Layout
It should be divided vertically in three spaces. 

Above them, there should be a logo with "Scriptor". The exact thing needs to be defined but let's put a placeholder there for now. 

Below the logo the definition should be the following 

Codex | Folio | Fragmentum
      |       | 

Each of these should be a list the user can scroll

There has to be a database selector, similar to what is already in the judo code, possibly placed somewhere else. 
The user should be able to alter the default values (those in src/tui/db/config.rs) by pressing Esc from the main menu (assuming no other pop up is opened). 
This shall open up a window on top of the tui, like in a retro videogame, where the use can select other defaults. Use btop as inspiration for this. 

## Theme

The theme (colours) must be read directly from the config file, see src/tui/db/config.rs for more details. 
Whenever possible there must be graphical elements that remind the user of medieval ages. 

## Key Bindings

The uses must be able to move vertically and horizontally using the arrows. Moving horizontally means moving between codex, folio and fragmentum. Moving vertically means moving inside codex or folio etc.

The user must be able to init a new codex (project) with "A" (capital A). The pop up conceptually identical to that already present should be used to insert the name

The user must be able to start a recording (a folio, provided a codex is selected) using "r". The recording also triggers the transcription as detailed above. 

The user must be able to add a recording (folio) from disk with "a". A pop-up will open up asking the path to the file. Transcription will then follow as usual. 

Provided a folio is selected, the user should be able to use "C" to copy all the entire fragmenta corresponding to it to the clipboard. 

Provided a fragmentum is selected, the user should be able to use "c" to copy just that fragmentum to the clipboard. 

The user can exit the app with q. 

The user can delete a codex and all associated stuff with "D".  The implementation should be identical to that already present.

A user can delete a folio and associated stuff with "d".   The implementation should be identical to that already present.

A user should be able tpo modify a codex with "M"  The implementation should be identical to that already present.
A user should be able to modify a folio with "m"  The implementation should be identical to that already present.

The user should be able to switch database with tab. The implementation should be identical to that already present.

There should be hints to commands (not of the arrows) in the relative sections. 

