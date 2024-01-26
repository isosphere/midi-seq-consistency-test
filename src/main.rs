// code was started from the midir example "test_read_input.rs"

use std::error::Error;
use std::fmt;
use std::io::{stdin, stdout, Write};

use midir::{Ignore, MidiInput};

fn main() {
    println!("Please configure your MIDI device to send MIDI clock messages to this program.");
    println!("Please send a sixteen-step sequence of MIDI notes, with one note per step, starting at middle C (MIDI index 60) and never repeating that note except at the same step.");
    println!("Issue a STOP transport command to see collected statistics.");

    match run() {
        Ok(_) => (),
        Err(err) => println!("Error: {}", err),
    }
}

#[derive(Debug)]
struct Step {
    note: u8,
    velocity: Vec<u8>,
    duration: Vec<u8>,
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let min_duration = self.duration.iter().min().unwrap();
        let max_duration = self.duration.iter().max().unwrap();
        let mut mean_duration = 0.0;
        for duration in self.duration.iter() {
            mean_duration += *duration as f32;
        }
        mean_duration /= self.duration.len() as f32;

        // find standard deviation of duration
        let mut variance = 0.0;
        for duration in self.duration.iter() {
            variance += (*duration as f32 - mean_duration).powi(2);
        }
        variance /= self.duration.len() as f32;
        let std_dev = variance.sqrt();

        write!(f, "Note: {}, Duration: {}-{} μs (mean {:.0} μs, σ = {:.0} μs, n = {})", self.note, min_duration, max_duration, mean_duration, std_dev, self.duration.len())
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut input = String::new();

    let mut midi_in = MidiInput::new("midir reading input")?;
    midi_in.ignore(Ignore::None);

    // Get an input port (read from console if multiple are available)
    let in_ports = midi_in.ports();
    let in_port = match in_ports.len() {
        0 => return Err("no input port found".into()),
        1 => {
            println!(
                "Choosing the only available input port: {}",
                midi_in.port_name(&in_ports[0]).unwrap()
            );
            &in_ports[0]
        }
        _ => {
            println!("\nAvailable input ports:");
            for (i, p) in in_ports.iter().enumerate() {
                println!("{}: {}", i, midi_in.port_name(p).unwrap());
            }
            print!("Please select input port: ");
            stdout().flush()?;
            let mut input = String::new();
            stdin().read_line(&mut input)?;
            in_ports
                .get(input.trim().parse::<usize>()?)
                .ok_or("invalid input port selected")?
        }
    };

    println!("\nOpening connection");
    let in_port_name = midi_in.port_name(in_port)?;

    let mut recorded_steps: [Option<Step>; 16] = [(); 16].map(|_| None);

    let mut current_step: Option<usize> = None; // zero-indexed, max value 15 corresponding to step 16
    let mut current_note_start: Option<u64> = None; // timestamp of when the current note was started, for computing duration

    // _conn_in needs to be a named parameter, because it needs to be kept alive until the end of the scope
    // reference used: https://computermusicresource.com/MIDI.Commands.html

    let _conn_in = midi_in.connect(
        in_port,
        "midir-read-input",
        move |stamp, message, _| {
            match message {
                &[250] => println!("TRANSPORT: Start"),
                &[251] => println!("TRANSPORT: Continue"),
                &[248] => {
                    //println!("TRANSPORT: Clock")
                },
                &[252] => {
                    for (i, step) in recorded_steps.iter().enumerate() {
                        if let Some(step) = step {
                            println!("Step {}: {}", i+1, step);
                        }
                    }
                },
                // Note ON
                &[144..=159, note, vel] => {
                    current_note_start = Some(stamp);

                    if note == 60 && current_step.is_none() {
                        current_step = Some(0);
                    }
                    
                    if current_step.is_none() {
                        println!("Note {} played before sequence started - waiting for 60 (middle C)", note);

                        // no-op
                    } else {
                        let current_step = current_step.unwrap();

                        assert!(note != 60 || current_step == 0, "Note 60 (middle C) must only be played on the first step of the sequence");

                        if recorded_steps[current_step].is_none() {
                            recorded_steps[current_step] = Some(Step {
                                velocity: vec![vel],
                                duration: vec![],
                                note
                            });
                            
                        } else {
                            recorded_steps[current_step].as_mut().unwrap().note = note;
                            recorded_steps[current_step].as_mut().unwrap().velocity.push(vel);
                        }
                    }
                },
                // Note OFF
                &[128..=143, note, _vel] => {
                    let prior_note = recorded_steps[current_step.unwrap()].as_ref().unwrap().note;
                    assert!(prior_note == note, "Note off {} on step {} does not match the prior note {}", note, current_step.unwrap(), prior_note);

                    if current_step.is_none() {
                        // no-op
                    } else {
                        let duration = stamp - current_note_start.unwrap();
                        current_note_start = None;
                        recorded_steps[current_step.unwrap()].as_mut().unwrap().duration.push(duration as u8);

                        current_step = Some((current_step.unwrap() + 1) % 16);
                    }
                },
                &[command, note, velocity] => {
                },
                _ => println!("{}: {:?} (len = {})", stamp, message, message.len()),
            }
        },
        (),
    )?;

    println!(
        "Connection open, reading input from '{}' (press enter to exit) ...",
        in_port_name
    );

    input.clear();
    stdin().read_line(&mut input)?; // wait for next enter key press

    println!("Closing connection");

    Ok(())
}