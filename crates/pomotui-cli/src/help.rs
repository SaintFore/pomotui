const ROOT: &str = "Pomotui controls the Timer Service and queries recorded focus.

Usage: pomotui [--json] <command>

Queries:
  activity [RANGE]  Focus Sessions and their recorded Session Reviews
  status            Current Session and timer state
  history           Full Session History, including breaks
  summary           Today's focus totals

Other commands:
  start, pause, resume, stop, skip  Control Sessions
  task, review, chain, reward       Manage Tasks, reviews, chains and rewards
  reserve, reservation              Manage Focus Reservations
  sync                              Manage synchronization
  waybar                            Print the Waybar status object
  fresh-start, reset                Reset or repair Pomotui data

Use 'pomotui <command> --help' for command syntax.
Use 'pomotui activity today --json' to gather records for a daily review.
";

fn command_help(command: &str) -> Option<&'static str> {
    Some(match command {
        "activity" => {
            "Usage: pomotui activity [today|yesterday|last-7-days] [--json]

List ended Focus Sessions by local end date, including Task, actual time,
outcome and any retained Session Review and Reflection.
The default is today. Last 7 days includes today and the previous 6 days.
With --json, read the date range and sessions from the response's value.

Examples:
  pomotui activity today --json
  pomotui activity yesterday --json
  pomotui activity last-7-days --json
"
        }
        "status" => "Usage: pomotui status [--json]\n\nShow the Current Session and timer state.\n",
        "history" => {
            "Usage: pomotui history [--json]\n\nList all retained Session History. For dated Focus Sessions and reviews, see 'pomotui activity --help'.\n"
        }
        "summary" => {
            "Usage: pomotui summary [--json]\n\nShow today's total and average focus time and number of Completed Rounds.\n"
        }
        "start" => {
            "Usage: pomotui start focus [--task ID | --title TITLE]
       pomotui start <short-break|long-break>
"
        }
        "pause" => "Usage: pomotui pause\n\nPause the Running Session.\n",
        "resume" => "Usage: pomotui resume\n\nResume the Paused Session.\n",
        "stop" => {
            "Usage: pomotui stop\n\nStop the Current Session. Stopping Focus early ends the Action Chain.\n"
        }
        "skip" => "Usage: pomotui skip\n\nSkip the Pending Session.\n",
        "task" => {
            "Usage: pomotui task list
       pomotui task create TITLE
       pomotui task rename ID TITLE
       pomotui task <complete|reopen|delete> ID
"
        }
        "review" => {
            "Usage: pomotui review success [--reflection TEXT]
       pomotui review success --task ID
       pomotui review success --void TITLE [--reflection TEXT]
       pomotui review failure [--task ID | --void TITLE] REFLECTION
       pomotui review dismiss
"
        }
        "chain" => {
            "Usage: pomotui chain
       pomotui chain archive
       pomotui chain edit ID --title TITLE [--reflection TEXT]
       pomotui chain edit ID --reflection TEXT
"
        }
        "reward" => {
            "Usage: pomotui reward list
       pomotui reward create THRESHOLD NAME [--budget BUDGET]
       pomotui reward update ID THRESHOLD NAME
       pomotui reward <delete|claim> ID
"
        }
        "reserve" => "Usage: pomotui reserve [--minutes MINUTES] [--task ID | --no-task]\n",
        "reservation" => {
            "Usage: pomotui reservation status
       pomotui reservation [cancel] success ID
       pomotui reservation [cancel] failure ID [REFLECTION] [--confirm]
       pomotui reservation minutes MINUTES
"
        }
        "sync" => {
            "Usage: pomotui sync enable PATH
       pomotui sync <disable|now|rebuild|status>
"
        }
        "waybar" => "Usage: pomotui waybar\n\nPrint a Waybar custom-module status object.\n",
        "fresh-start" => {
            "Usage: pomotui fresh-start --confirm\n\nReset Pomotui data across synchronized Devices.\n"
        }
        "reset" => {
            "Usage: pomotui reset --all-data --confirm\n\nBack up and remove the local database for repair while the Timer Service is stopped.\n"
        }
        _ => return None,
    })
}

#[must_use]
pub fn help(args: &[String]) -> Option<&'static str> {
    let mut words = args
        .iter()
        .map(String::as_str)
        .filter(|word| *word != "--json");
    match (words.next(), words.next(), words.next()) {
        (None | Some("--help" | "-h"), None, None) => Some(ROOT),
        (Some(command), Some("--help" | "-h"), None) => command_help(command),
        _ => None,
    }
}

pub(crate) fn usage_error(words: &[&str]) -> String {
    match words.first().and_then(|command| command_help(command)) {
        Some(help) => help.trim_end().into(),
        None => {
            "Usage: pomotui [--json] <command>\nRun 'pomotui --help' for available commands.".into()
        }
    }
}
