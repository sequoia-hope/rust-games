//! A headless run with CPU riders, for tuning: `cargo run --release -p sb_sim --bin sb-sim -- [course] [mode]`.

use sb_sim::course::CourseId;
use sb_sim::race::{Mode, Session, SessionEvent, State};
use sb_sim::rider::{Event, Input};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let course = args
        .first()
        .and_then(|k| CourseId::from_key(k))
        .unwrap_or(CourseId::Crystal);
    let mode = args.get(1).and_then(|k| Mode::from_key(k)).unwrap_or(Mode::Race);
    let mut s = Session::new(course, mode, 0, true);
    println!(
        "{}: {:.0} m, drop {:.0} m, {} kickers, {} obstacles",
        course.name(),
        s.course.finish_s - s.course.start_s,
        s.course.world(s.course.start_s, 0.0).y - s.course.world(s.course.finish_s, 0.0).y,
        s.course.kickers.len(),
        s.course.obstacles.len()
    );
    let mut top = 0.0f32;
    let mut steps = 0;
    while s.state != State::Done && steps < 120 * 600 {
        s.step(Input::default());
        top = top.max(s.player().rider.speed());
        steps += 1;
        for e in s.events.drain(..) {
            match e {
                SessionEvent::Rider(who, Event::Trick(t)) => {
                    println!("{:7.2}s  {who}  {} +{}", s.time, t.name, t.points)
                }
                SessionEvent::Rider(who, Event::Crash { tree }) => println!(
                    "{:7.2}s  {who}  crash{} at s {:.0}",
                    s.time,
                    if tree { " (tree)" } else { "" },
                    s.entrants[who].rider.s
                ),
                SessionEvent::Rider(who, Event::Land { landing, impact }) => {
                    let r = &s.entrants[who].rider;
                    println!(
                        "{:7.2}s  {who}  land {landing:?} impact {impact:.1} air {:.2}s h {:.1}m",
                        s.time, r.air.time, r.air.height
                    )
                }
                SessionEvent::Finish { who, time } => println!("{time:7.2}s  {who}  FINISH"),
                _ => {}
            }
        }
    }
    for (i, e) in s.entrants.iter().enumerate() {
        println!(
            "{} {}: finish {:?}, score {}, crashes {}, place {}",
            i,
            e.name(),
            e.finish,
            e.score,
            e.crashes,
            s.place(i)
        );
    }
    println!("top speed {:.1} km/h", top * 3.6);
}
