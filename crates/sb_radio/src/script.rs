//! What the announcers say: Dani Reyes and Wren Halvorsen, the
//! commentary team on the television coverage of the Ten-Eighty Mountain
//! Series, calling every run live from the booth at the bottom of the hill.
//!
//! **Dani** calls the play-by-play: a retired halfpipe pro from New Jersey
//! with a voice built for big moments. She rises with every run, shouts the
//! landings, and has five broken collarbones and a nacho habit to show for
//! her career. **Wren** is the analyst: a New Zealander, younger, a
//! boardercross racer who retired early on bad knees. Quick, sharp and dry,
//! she has the stats on everything (a spreadsheet of every trick landed
//! this season), lights up for a clean line, and is softer than she lets
//! on. They raced each other for years and have been friends for longer
//! than either admits; they bicker like it, on air.
//!
//! It is television, not radio: they talk to the viewers at home, the
//! cameras, the crowd at the finish and the riders in the gate, with the
//! pace and lift of live sport. Every line has to be true of whatever just
//! happened: what is said about a run's result is chosen by how the run
//! went ([`crate::director`]), so a line that assumes something (a tree, a
//! clean run) belongs in the topic for it.
//!
//! A topic is a moment the director can speak to; a bit is one thing said
//! about it, one or more lines in turn; a line is one clip, spoken by one
//! of them. A clip's name comes from where it sits ([`crate::clip_id`]),
//! and `audio/announcers/index.json` records the text each clip was
//! recorded from, so an edited line is simply re-recorded by
//! `tools/announcers/render.py`; until then the caption shows and the clip
//! is skipped. Lines are written for the voice model: words, not digits;
//! no capitals for shouting; punctuation for energy.

use crate::Who::{Dani as D, Wren as W};

pub type Bit = &'static [(crate::Who, &'static str)];

/// Every topic and its bits, in a fixed order (clip names depend on it:
/// add new bits at the end of a topic).
pub const TOPICS: &[(&str, &[Bit])] = &[
    // The broadcast's open, in the menu.
    (
        "ident",
        &[
            &[
                (D, "Good afternoon, everybody, and welcome to live coverage of the Ten-Eighty Mountain Series! I'm Dani Reyes!"),
                (W, "And I'm Wren Halvorsen. Fast snow, a hungry field, and Dani has already had two coffees."),
                (D, "Three!"),
            ],
            &[
                (W, "Welcome back to the Ten-Eighty Mountain Series! Wren Halvorsen, alongside Dani Reyes."),
                (D, "Wren, it is a gorgeous day up here, and we have got a stacked field!"),
                (W, "We have four riders, Dani."),
                (D, "Stacked!"),
            ],
            &[
                (D, "You are watching the Ten-Eighty Mountain Series, live from the mountain! Four courses! Four riders! One booth!"),
                (W, "One booth, with a heater that smells like toast. We have told the network."),
            ],
            &[
                (W, "If you're just joining us, welcome! The riders are up top, choosing their courses right now."),
                (D, "And the crowd at the finish is loud, folks! I can hear the cowbells from the booth!"),
            ],
            &[
                (D, "Live from the summit, this is the Ten-Eighty Mountain Series! I'm Dani Reyes, and beside me, the sharpest analyst in the sport!"),
                (W, "Accurate."),
            ],
            &[
                (W, "Wren Halvorsen and Dani Reyes, back in the booth together!"),
                (D, "Fifteen years, folks! We used to race each other. Now we argue on television!"),
                (W, "On live television. Which is so much worse."),
            ],
        ],
    ),
    // Between runs, while the menu is up.
    (
        "menu",
        &[
            &[
                (D, "While we wait on the next rider, Wren, give the folks at home one thing to watch for!"),
                (W, "The landings. Anyone can spin. Not everyone can stop spinning."),
            ],
            &[
                (W, "A quick stat for the viewers. Dani broke her left collarbone five times in her career."),
                (D, "And I came back every single time!"),
                (W, "Very slowly."),
            ],
            &[
                (D, "Wren, you raced boardercross for, what, eight seasons?"),
                (W, "Seven. My knees voted on the eighth."),
            ],
            &[
                (W, "I keep a spreadsheet of every trick landed this season."),
                (D, "Folks, it is color coded! The network wants it on screen, and she will not let them!"),
            ],
            &[
                (W, "The truck is telling us we have a moment before the next run."),
                (D, "Which means Wren is going to talk about wax!"),
                (W, "Wax matters, Dani!"),
            ],
            &[
                (D, "Let's take you up to the start area! The riders are stretching, they're studying the course, they're getting focused!"),
                (W, "They're looking at their phones, Dani."),
            ],
            &[
                (W, "For the viewers at home: first one to the bottom wins. It's not complicated."),
                (D, "It's beautifully simple! That's why we love it!"),
            ],
            &[
                (D, "Now, we did have a moose wander past the finish area earlier."),
                (W, "Course security has asked him to leave."),
                (D, "His name is Gerald!"),
                (W, "We don't know that."),
            ],
            &[
                (W, "Conditions across the mountain! Sunshine on Crystal Ridge, golden light in the trees on Timber Run, a whiteout in Avalanche Bowl, and sunset in the pipe!"),
                (D, "Something for everybody today!"),
            ],
            &[
                (D, "Wren, who've you got today?"),
                (W, "Whoever lands their tricks."),
                (D, "That is not a pick!"),
                (W, "It's the only pick."),
            ],
            &[
                (D, "Big shout out to the queer ski club, out in force at the bottom of the course today!"),
                (W, "They brought a banner. It is very sparkly. The cameras love it."),
            ],
            &[
                (D, "The nachos at the lodge tonight, folks! I'm just saying! I'm just putting it out there!"),
                (W, "Dani will have eaten all of them by four."),
            ],
        ],
    ),
    // At the start of a run, one of these for the course...
    (
        "crystal",
        &[
            &[
                (D, "We are live at Crystal Ridge! Wide open groomers and big, friendly kickers!"),
                (W, "Friendly is a strong word. They're polite."),
            ],
            &[
                (W, "Crystal Ridge today! The sun is out, the snow is fast, and there are no excuses on this one."),
                (D, "None! Let's go!"),
            ],
            &[
                (D, "Crystal Ridge, and just look at that view from the top!"),
                (W, "Enjoy the view later. Watch the jumps now."),
            ],
        ],
    ),
    (
        "timber",
        &[
            &[
                (W, "Timber Run! Tight trees, and you have to pick your line early."),
                (D, "And folks, the trees do not move! I have checked! Personally! With my face!"),
            ],
            &[
                (D, "Timber Run, golden hour in the pines! Gorgeous pictures from up top!"),
                (W, "Very pretty. The trunks are very hard."),
            ],
            &[
                (W, "Timber Run is all about rhythm. Turn, turn, breathe."),
                (D, "Turn, turn, scream!"),
                (W, "That is not rhythm!"),
            ],
        ],
    ),
    (
        "avalanche",
        &[
            &[
                (D, "Avalanche Bowl! The steepest course on this mountain!"),
                (W, "Steep, fast, and huge air. In this visibility, you ride the flags."),
            ],
            &[
                (W, "Avalanche Bowl! This is the expert course."),
                (D, "And the snow is coming down sideways up there!"),
                (W, "Our camera crew is very, very brave."),
            ],
            &[
                (D, "Avalanche Bowl, in a whiteout! The cameras can barely see!"),
                (W, "Neither can the rider. Follow the flags!"),
            ],
        ],
    ),
    (
        "pipe",
        &[
            &[
                (D, "The superpipe! Folks, I spent ten years in this thing!"),
                (W, "Mostly on the landings."),
            ],
            &[
                (W, "The halfpipe! Wall to wall: height, rotation, and a clean landing."),
                (D, "On both feet, folks! Very important! Both of them!"),
            ],
            &[
                (D, "Sunset in the pipe! Let's see some spins!"),
                (W, "Clean spins. Rotation without a landing is just falling with ambition."),
            ],
        ],
    ),
    // ...or for the rider.
    (
        "kai",
        &[
            &[
                (D, "In the gate, it's Kai! The all-rounder!"),
                (W, "Good at everything, great at nothing. A very consistent career."),
            ],
            &[
                (W, "Kai at the top! Steady hands, steady board."),
                (D, "Steady as they come!"),
            ],
        ],
    ),
    (
        "mika",
        &[
            &[
                (W, "Mika's in the start gate! A technical rider, and very clean."),
                (D, "Mika's the only athlete on tour who actually reads Wren's spreadsheet!"),
                (W, "My favorite."),
            ],
            &[(D, "Here's Mika! Nobody on tour lands cleaner!")],
        ],
    ),
    (
        "rob",
        &[
            &[
                (D, "Here comes Rob, and Rob is fast!"),
                (W, "Rob turns maybe twice a run. Both by accident."),
            ],
            &[
                (W, "Rob at the top! A pure speed rider. Tuck, point, and hope."),
                (D, "Hope is a strategy for Rob!"),
            ],
        ],
    ),
    (
        "jules",
        &[
            &[
                (D, "Jules is in the gate! The big air specialist!"),
                (W, "Jules jumps first and does the math in the air."),
            ],
            &[
                (W, "Jules at the top! Expect altitude."),
                (D, "Expect the cameras to lose the shot!"),
            ],
        ],
    ),
    (
        "go",
        &[
            &[(D, "And we are underway!")],
            &[(W, "Clean out of the gate!")],
            &[(D, "Here we go!")],
            &[(D, "Send it!"), (W, "Within reason!")],
            &[(W, "Good start! Good start!")],
            &[(D, "And they're away!")],
        ],
    ),
    (
        "trick",
        &[
            &[(D, "Oh, nice!")],
            &[(W, "Clean!")],
            &[(D, "Look at the style on that!")],
            &[(W, "Tidy! I'll allow it.")],
            &[(D, "Yes! Stomped it!")],
            &[(W, "That one's going in the spreadsheet!")],
            &[(D, "Beautiful! Just beautiful!")],
            &[(W, "Smooth in, smooth out. Lovely.")],
        ],
    ),
    (
        "big",
        &[
            &[(D, "Oh my goodness! Did you see that?"), (W, "I saw it! I'm still processing it!")],
            &[(W, "That is a big one! Write that down!"), (D, "That is going straight on the highlight reel!")],
            &[(D, "Absolutely massive! The crowd is on its feet!"), (W, "So is Dani.")],
            &[(W, "Okay. That was genuinely impressive."), (D, "She said genuinely! Folks, that never happens!")],
            &[(D, "Huge! Absolutely huge!")],
            &[(W, "Textbook! Somebody put that in the training video!")],
        ],
    ),
    (
        "sloppy",
        &[
            &[(W, "Ooh, sketchy landing!"), (D, "But rides away!")],
            &[(D, "Little wobble there!"), (W, "Big wobble.")],
            &[(W, "Landed! Technically.")],
            &[(D, "Saved it! Mostly!")],
        ],
    ),
    (
        "crash",
        &[
            &[(D, "Oh! And that is a yard sale!")],
            &[(W, "And that's a fall."), (D, "Thank you, Wren!")],
            &[(D, "Ooh, that's going to leave a mark!")],
            &[(W, "Down they go! Up you get, keep it moving!")],
            &[(D, "Wipeout! That is snow in places snow should never be!")],
            &[(W, "That one got away. Happens to the best of them."), (D, "Happened to me constantly!")],
            &[(D, "Shake it off! Back up and going!")],
        ],
    ),
    (
        "tree",
        &[
            &[(D, "Timber!"), (W, "The tree is fine, by the way.")],
            &[(W, "Found a tree!"), (D, "The trees always win, folks!")],
            &[(D, "Oh no, right into the pines!"), (W, "The pines were here first.")],
            &[(W, "That's a tree. The tree did not move."), (D, "They never do!")],
        ],
    ),
    (
        "lead",
        &[
            &[(D, "And that's the lead! Out in front!")],
            &[(W, "A pass, and a very clean one!")],
            &[(D, "Into first place! Go, go, go!")],
            &[(W, "Takes the lead! Now don't give it back.")],
        ],
    ),
    (
        "behind",
        &[
            &[(W, "Oh, and that's a pass! Down to second.")],
            &[(D, "The lead is gone! Come on, come on!")],
            &[(W, "Trailing now. Plenty of mountain left!")],
            &[(D, "No, no, no! Chase it down!")],
        ],
    ),
    (
        "fast",
        &[
            &[(D, "Look at the speed!"), (W, "Full tuck! Very aerodynamic, very brave!")],
            &[(W, "That's very fast. Too fast. Perfect.")],
            &[(D, "Full tuck! The speed gun is going crazy!")],
        ],
    ),
    (
        "air",
        &[
            &[(D, "Big air!")],
            &[(W, "Air traffic control is calling!")],
            &[(D, "Still flying!"), (W, "Still flying!")],
        ],
    ),
    // When nothing has happened for a while in a run.
    (
        "idle",
        &[
            &[(W, "Good lines so far!"), (D, "Lovely lines! I love a line!")],
            &[(D, "Wren, what's for dinner tonight?"), (W, "We are on air."), (D, "So what's for dinner, on air?")],
            &[(W, "Watch the knees. Soft knees soak up everything."), (D, "Soft knees, soft heart!")],
            &[(D, "This is my favorite section of the course!"), (W, "You say that about every section.")],
            &[(W, "There are free points on those kickers!"), (D, "Free points are the best points!")],
            &[(D, "I am so proud of every athlete out here today!"), (W, "It's been four minutes."), (D, "Proud!")],
            &[(W, "The longer you hold a grab, the more it scores."), (D, "Grab it like you mean it!")],
            &[(D, "Wren, sum up this run in one word!"), (W, "Downhill."), (D, "Poetry, folks!")],
            &[(W, "Viewers at home, do not try this in your driveway."), (D, "Unless your driveway is a mountain!")],
        ],
    ),
    // The rival finished first, the player is still riding.
    (
        "rival",
        &[
            &[(W, "The rival is across the line! Keep pushing, all the way down!")],
            &[(D, "The rival's in! Don't give up, finish it out!")],
        ],
    ),
    // A win in a race...
    (
        "win",
        &[
            &[(D, "And that's the win! What a race!"), (W, "Well raced. Genuinely.")],
            &[(W, "First over the line!"), (D, "I'm getting emotional, folks! I am actually getting emotional!")],
            &[(D, "Victory! Somebody get this rider a podium and a plate of nachos!")],
            &[(W, "That is how you race!"), (D, "That is how you race!")],
        ],
    ),
    // ...with nothing crashed on the way down.
    (
        "win-clean",
        &[
            &[(W, "First place, and not a single fall! That is a clean run!"), (D, "Clean as fresh snow, folks!")],
            &[(D, "The win, and they never touched the snow! Wren, did you see that?"), (W, "Didn't put a foot wrong. That's going in the spreadsheet!")],
        ],
    ),
    // A loss in a race...
    (
        "lose",
        &[
            &[(W, "Second place today."), (D, "Run it back! Run it back!")],
            &[(D, "Ooh, not quite!"), (W, "Not today. There's always another run.")],
            &[(W, "Second across the line."), (D, "Rematch! We need a rematch!")],
            &[(D, "Second place is the first loser! I'm kidding! I'm kidding, it's great!"), (W, "Dani.")],
            &[(W, "The rival takes this one."), (D, "Heads up! That was a good fight!")],
        ],
    ),
    // ...after hitting a tree on the way down.
    (
        "lose-trees",
        &[
            &[(D, "Ooh, just missed it!"), (W, "Next time, fewer trees.")],
            &[(W, "Second place. And the trees had a say in that one."), (D, "The trees are undefeated this season, folks!")],
            &[(D, "Not today! But hey, the tree's okay!"), (W, "The tree has a better record than most of the field.")],
        ],
    ),
    // The end of a time attack...
    (
        "time",
        &[
            &[(W, "Across the line! Check the clock!"), (D, "Is that a record? It feels like a record!")],
            &[(D, "And that's the finish! What a run!"), (W, "The time is on the board.")],
            &[(W, "Done! Now do it faster."), (D, "She means that lovingly, folks!")],
        ],
    ),
    // ...and of a trick attack.
    (
        "score",
        &[
            &[(D, "What a show! Look at that score!"), (W, "Points on the board!")],
            &[(W, "And that's the trick attack! Let's see the total."), (D, "I'm dizzy just watching!")],
            &[(D, "That's the session! Style for days!"), (W, "Style for one whole run. But good style.")],
        ],
    ),
    (
        "restart",
        &[
            &[(D, "And we are going again!")],
            &[(W, "Another attempt! Good.")],
            &[(D, "Back to the top! One more time!")],
        ],
    ),
];
