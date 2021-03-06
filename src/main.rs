extern crate Jarvis;
extern crate num_cpus;
extern crate termcolor;

use std::io::Write;
use std::sync::{
    mpsc, mpsc::{Receiver, Sender}, Arc, Mutex,
};
use std::thread;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use Jarvis::Communication::Channel;
use Jarvis::Device::Command;
use Jarvis::Device::Command::{CommandExecution, CommandListen};
use Jarvis::Device::Device;

enum message_level {
    info,
    warning,
    error,
    success,
}

enum execution_order {
    sync,
    async,
}

fn post_message(message: &str, level: message_level) {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    let log_message = match level {
        message_level::warning => ("Warning", Color::Yellow),
        message_level::error => ("Error", Color::Red),
        message_level::success => ("OK", Color::Green),
        _ => ("Info", Color::White),
    };
    stdout.set_color(ColorSpec::new().set_fg(Some(log_message.1)));
    writeln!(&mut stdout, "{}", [log_message.0, ":", message].join(""));
}
fn listen_to_commands(com_channel: Channel::Channel) {
    post_message("ENTER OR VOICE COMMAND", message_level::info);
    std::io::stdout().flush();

    //use tuple of (io,execution_order) to determine execution of application
    let channel = mpsc::channel();
    let io_execution = vec![(Command::TextInput::new(""), execution_order::sync)];
    let thread_data = Arc::new(com_channel);
    let th = thread_data.clone();
    let send_clone_channel = channel.0.clone();
    let results = io_execution
        .into_iter()
        .map(move |execution| match execution.1 {
            execution_order::async => Some(thread::spawn(move || -> () {
                loop {
                    let mut text_io = Command::TextInput::new("");
                    let exec = text_io.listen(&th);
                    match exec {
                        Ok(command) => match send_clone_channel.send(command) {
                            Ok(com) => {}
                            Err(_) => {
                                post_message("CHANNEL COULD NOT BE SEND", message_level::error);
                            }
                        },
                        Err(_) => {
                            post_message("COULD NOT EXECUTE COMMAND", message_level::error);
                        }
                    }
                }
            })),
            _ => None,
        })
        .collect::<Vec<_>>();

    let tcount = thread_data.clone();
    let loop_result = thread::spawn(move || -> () {
        let thr = tcount.clone();
        loop {
            match channel.1.recv() {
                Ok(exec) => {
                    exec.execute(&thr);
                }
                Err(_) => {
                    post_message("COULD NOT RECIEVE COMMAND", message_level::error);
                }
            }
        }
    });

    results
        .into_iter()
        .map(|f| match f {
            Some(res) => {
                res.join();
            }
            _ => (),
        })
        .collect::<Vec<_>>();
    loop_result.join();
}

fn main() {
    post_message("Starting JARVIS", message_level::info);
    post_message("Checking for devices", message_level::info);
    println!("=====================");
    let com_channel = Channel::Channel::new("0.0.0.0:61000", "0.0.0.0:62345");
    match Device::set_up_devices(&com_channel) {
        Ok(devices) => {
            println!("Set up devices : devices {:?}", devices);
            listen_to_commands(com_channel);
        }
        Err(err) => {
            post_message("JARVIS COULD NOT START", message_level::error);
            listen_to_commands(com_channel);
        }
    };
    println!("=====================");
}
