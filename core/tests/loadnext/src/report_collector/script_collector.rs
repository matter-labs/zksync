// use futures::channel::mpsc::Receiver;

/// Script collector is an entity capable of recording all the
/// actions that mutate the server state in a form that can be later reproduced.
///
/// Since the actions in the loadtest are randomly generated, it is important to
/// make scenarios reproducible, just to be able to reproduce the bug discovered
/// during the test run.
///
/// Script collector logic is divided into two parts:
/// - Report acceptor interface, capable of taking action reports, transforming them
///   into the expected form, and sending to the storing actor.
/// - Storing actor, writing all the actions to the file.
///
/// This is required in order to both not introduce more channels in public interfaces
/// and not make reporting function too heavy because of the file IO operations.
#[derive(Debug)]
pub struct ScriptCollector {}

// #[derive(Debug)]
// struct ScriptSaver {
//     sink: Receiver<>
// }
