use super::filter::should_moderate;

fn kws(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// Run a moderation decision table. Each case is `(texts, keywords, expected)`:
/// every text in `texts` is checked against the `keywords` blocklist and must
/// yield `expected` — `Some(keyword)` for a message that is moderated (the
/// keyword string `should_moderate` returns) or `None` for one that is allowed.
fn check_cases(cases: &[(&[&str], &[&str], Option<&str>)]) {
    for &(texts, keywords, expected) in cases {
        for &text in texts {
            assert_eq!(
                should_moderate(text, &kws(keywords)).as_deref(),
                expected,
                "text={text:?} keywords={keywords:?}"
            );
        }
    }
}

#[test]
fn should_moderate_table() {
    // Real-world false positive: "a55"/"ass" used to flag this prose because
    // full flood-collapsing folded "ass" → "as" and the merge heuristic glued
    // "as" onto a neighbouring "s".
    let prose = "So I was testing the self destruct feature on my alt account and \
                 it doesn't actually delete the account? I can still see it \
                 unconnected to the various groups it was in and I can still \
                 message it as well.";
    // dog / doggy / doggie (+ style, joined or split) — mutually equivalent.
    let doggy: &[&str] = &[
        "doggy style",
        "doggie style",
        "dogy style",
        "doggystyle",
        "doggiestyle",
        "doggies style",
        "dog style",
    ];
    // tit / tits / titty / titties — mutually equivalent.
    let tit: &[&str] = &[
        "look at the tit over there",
        "look at the tits over there",
        "look at the titty over there",
        "look at the titties over there",
    ];
    // Ordinary, legitimate messages that must never be moderated even though a
    // blocked keyword is a substring of an innocent longer word.
    check_cases(&[
        // --- trivial / sanity ---
        (&[""], &["spam"], None),                           // empty text
        (&["hello world"], &[], None),                      // empty keyword list
        (&["hello"], &["", "   ", "\t"], None),             // whitespace-only keywords ignored
        (&["!!! ??? ..."], &["spam"], None),                // text without letters
        (&["the quick brown fox"], &["spam", "ads"], None), // no keyword present
        // --- basic EN matching ---
        (&["hello world", "HELLO"], &["hello"], Some("hello")),
        (&["the quick brown fox"], &["lazy", "fox"], Some("fox")), // first match returned
        (&["Hello WORLD", "hello, world!"], &["world"], Some("world")), // case / punctuation
        (&["hello"], &["HELLO"], Some("HELLO")),
        (&["(spam)."], &["spam"], Some("spam")),
        (&["end-of-line"], &["line"], Some("line")),
        (
            &["buy cheap pills now"],
            &["cheap pills"],
            Some("cheap pills"),
        ), // multi-word, contiguous
        (&["cheap and pills"], &["cheap pills"], None),
        (&["classic music"], &["ass"], None), // no substring match in ordinary words
        (&["therapist"], &["rapist"], None),
        // --- doubled letters must be preserved ("butt" vs "but"): a
        //     doubled-letter keyword needs *at least* that many repeats ---
        (
            &[
                "I cannot scroll back but when I click nothing happens",
                "but",
            ],
            &["butt"],
            None,
        ),
        (&["pas de deux"], &["pass"], None), // "pass" (ss) vs "pas"
        (&["say helo"], &["hello"], None),   // "hello" (ll) vs "helo"
        // ...but the real word still matches, incl. a flooded non-doubled letter.
        (
            &["what a butt", "nice butt!", "buuutt", "greeeat butt"],
            &["butt"],
            Some("butt"),
        ),
        // Keyword "boob" (doubled `o`) catches heavily flooded / extended forms
        // and the `-y` → `-ies` plural "boobies" (incl. flood / leet bypasses)...
        (
            &[
                "boob",
                "booob",
                "booooobs everywhere",
                "booooob",
                "boooooobb",
                "look at the boobies",
                "BOOBIES",
                "b00bies everywhere",
                "booooobies",
            ],
            &["boob"],
            Some("boob"),
        ),
        // ...but never on unrelated words that merely share letters.
        (&["the job is done", "a big bob"], &["boob"], None),
        // --- doubled letters in the *keyword* must not collapse onto innocent
        //     words ("ass"/"a55" matched "as" runs in normal prose) ---
        (&[prose], &["a55"], None),
        (
            &[prose, "as well as before", "as soon as possible"],
            &["ass"],
            None,
        ),
        (&["don't be an ass", "such an a55"], &["ass"], Some("ass")),
        // --- basic RU matching ---
        (&["привет мир", "ПРИВЕТ"], &["привет"], Some("привет")),
        (&["Привет"], &["ПРИВЕТ"], Some("ПРИВЕТ")),
        (&["привет, мир!"], &["мир"], Some("мир")),
        (&["(спам)."], &["спам"], Some("спам")),
        (&["дешёвые и таблетки"], &["дешёвые таблетки"], None),
        (&["ёлка"], &["елка"], Some("елка")), // ё ≡ е on either side
        (&["елка"], &["ёлка"], Some("ёлка")),
        (&["барак обама"], &["рак"], None), // no substring match ("рак" in "барак")
        // --- bypass: separators / repeats / leet / @-mention / combined (EN) ---
        (
            &[
                "watch out: s p a m incoming",
                "s.p.a.m here",
                "s-p-a-m here",
                "s_p_a_m here",
                "s. p-a_m!",
                "spaaaaam everywhere",
                "ssssspppaaammm",
                "5p4m incoming",
                "$pam",
                "5-p-4-a-m",
                "5 p 4 4 m",
                "@spam",
                "5P@M",
                "5--p..aaa  M!!!",
                "buy spams now",
                "5-p-a-m-s incoming",
                "s p a m s",
                "spaaaams",
            ],
            &["spam"],
            Some("spam"),
        ),
        // --- bypass (RU), incl. cyrillic look-alikes and a plural ---
        (
            &[
                "с п а м прямо тут",
                "с.п.а.м здесь",
                "с-п-а-м здесь",
                "спаааам везде",
                "сообщение: спам",
                "с-п-аааа-м!!!",
                "это спам",
            ],
            &["спам"],
            Some("спам"),
        ),
        (&["spam here"], &["5p4m"], Some("5p4m")), // keyword itself stored in leet
        (
            &["ping @crawlerbot now"],
            &["crawlerbot"],
            Some("crawlerbot"),
        ), // mention sigil
        (&["this is a scаm offer"], &["scam"], Some("scam")), // cyrillic а
        (&["помогите cоc!"], &["сос"], Some("сос")), // latin c, o, c
        (
            &[
                "купи дешёвые таблетки сейчас",
                "купи д.е.ш.ё.в.ы.е таблетки сейчас",
            ],
            &["дешёвые таблетки"],
            Some("дешёвые таблетки"),
        ),
        // --- negatives that must NOT be flagged ---
        (&["I am at my own home"], &["spam"], None), // legit short words must not merge
        (&["spam"], &["spamster"], None),            // partial overlap
        (&["hi"], &["hello there friend"], None),    // keyword longer than text
        // --- english plural handling (singular ⇄ plural on either side) ---
        (&["look at the boxes"], &["box"], Some("box")),
        (&["two parties tonight"], &["party"], Some("party")),
        (&["this is spam"], &["spams"], Some("spams")),
        (&["one box only"], &["boxes"], Some("boxes")),
        (&["the party"], &["parties"], Some("parties")),
        (&["the dishes are clean"], &["dish"], Some("dish")), // -es after sibilant cluster
        (&["two churches"], &["church"], Some("church")),
        (&["many quizzes"], &["quiz"], Some("quiz")),
        (&["stop calling them bitches"], &["bitch"], Some("bitch")),
        // short words ending in `s` must NOT be stripped (to "bu"/"ga"/"ad")
        (&["the bus is late"], &["bu"], None),
        (&["no gas left"], &["ga"], None),
        (&["see the ads"], &["ad"], None),
        // --- morphology: compound (joined/split), derivational & diminutive ---
        (&["that was a blow job"], &["blowjob"], Some("blowjob")),
        (&["blowjob here"], &["blow job"], Some("blow job")),
        (
            &["stop masturbation talk", "he is masturbating"],
            &["masturbate"],
            Some("masturbate"),
        ),
        (&["pink panties"], &["panty"], Some("panty")),
        (&["so sexy tonight"], &["sex"], Some("sex")),
        (
            &["acting sexually", "about sexuality"],
            &["sexual"],
            Some("sexual"),
        ),
        // dog/doggy/doggie and tit families: every form matches every spelling
        (doggy, &["doggie style"], Some("doggie style")),
        (doggy, &["doggy style"], Some("doggy style")),
        (doggy, &["doggystyle"], Some("doggystyle")),
        (doggy, &["doggiestyle"], Some("doggiestyle")),
        (doggy, &["dog style"], Some("dog style")),
        (tit, &["tit"], Some("tit")),
        (tit, &["tits"], Some("tits")),
        (tit, &["titty"], Some("titty")),
        (tit, &["titties"], Some("titties")),
        // curated equivalence groups must not over-match unrelated words
        (&["the sexton rang the bell"], &["sex"], None),
        (&["a homosexual rights march"], &["sexual"], None),
        (&["the titanic sank in 1912"], &["tit"], None),
        (&["a dogma is a fixed belief"], &["dog style"], None),
        (&["the masterpiece was stunning"], &["masturbate"], None),
        // --- false-positive guards against a realistic blocklist ---
        (
            &[
                // "ass" hiding in ordinary words
                "the class starts at noon",
                "please pass the salt",
                "a brass band played",
                "the embassy is closed today",
                "let me assess the situation",
                "I assume you are coming",
                // "sex" hiding in place / ordinary words
                "we drove through Essex on holiday",
                "the sextant is a navigation tool",
                "she studies middlesex county records",
                // "tit" hiding in ordinary words
                "the title of the book is great",
                "the constitution protects rights",
                "I signed the online petition",
                "competition was fierce this year",
                "his attitude has improved",
                // "anal" hiding in ordinary words
                "the data analysis looks correct",
                "we need to analyze the logs",
                "the canal boat trip was lovely",
                // "cum" hiding in ordinary words
                "the cucumber salad was fresh",
                "please read the documentation",
                "under the circumstances we agree",
                "savings accumulate over time",
                // "hell" hiding in ordinary words
                "she sells sea shells",
                "the turtle hid in its shell",
                // "dog" near "style" but unrelated meaning
                "my dog has a nice coat",
                "I like that coding style",
                // everyday conversation
                "Hey everyone, what time is the meeting tomorrow?",
                "Thanks for the help, that fixed my bug!",
                "Could someone share the link to the docs?",
                "I just deployed the new version, please test it.",
                "Happy birthday! Hope you have a wonderful day.",
                "The weather is lovely today, perfect for a walk.",
                "Does anyone know a good restaurant nearby?",
                "I'll be out of office next week on vacation.",
                "Great presentation, really clear explanations.",
                "Can we reschedule the call to 3pm?",
                // code / urls
                "see https://example.com/assets/main.css for details",
                "run `cargo test --package bot` to verify",
                "the class ClassName implements Trait",
                "git commit -m \"fix: assertion in parser\"",
            ],
            &[
                "spam",
                "ass",
                "sex",
                "tit",
                "anal",
                "cum",
                "hell",
                "boob",
                "butt",
                "sexual",
                "masturbate",
                "blowjob",
                "dog style",
            ],
            None,
        ),
        (
            &[
                "барак был большой",    // contains "рак"
                "это просто сообщение", // guarded anyway
                "красивый закат сегодня",
                "сосна растёт в лесу", // "сос" inside "сосна"
            ],
            &["рак", "спам", "сос"],
            None,
        ),
    ]);
}
