//! Fixtures de diseño: nunca se mezclan con una sesión real ni salen a la red.
pub(super) struct Player {
    pub name: &'static str,
    pub agent: &'static str,
    pub rank: &'static str,
    pub kd: &'static str,
    pub wr: &'static str,
    pub form: &'static str,
    pub hs: &'static str,
    pub adr: &'static str,
    pub hidden: bool,
}

pub(super) struct Match {
    pub map: &'static str,
    pub agent: &'static str,
    pub score: &'static str,
    pub won: bool,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub rr: i32,
    pub acs: u32,
}

pub(super) struct Demo {
    pub players: Vec<Player>,
    pub rounds: Vec<(u32, Option<(u32, u32)>)>,
    pub matches: Vec<Match>,
    pub post: bool,
}

impl Default for Demo {
    fn default() -> Self {
        let rows = [
            (
                "Norte·tú",
                "Sova",
                "DIA 2",
                "1.18",
                "55%",
                "VVDVV",
                "26%",
                "152",
            ),
            (
                "Bruma", "Omen", "DIA 1", "1.06", "50%", "DVDVV", "22%", "138",
            ),
            (
                "Prisma", "Jett", "ASC 1", "1.31", "60%", "VVVDV", "31%", "167",
            ),
            (
                "Cobre", "Killjoy", "DIA 2", "0.98", "50%", "VDDVD", "24%", "126",
            ),
            (
                "Luna", "Sage", "PLA 3", "1.02", "55%", "DVVDV", "21%", "132",
            ),
            ("Eco", "Raze", "DIA 3", "1.22", "55%", "VVDDV", "27%", "160"),
            ("Oculto", "Cypher", "—", "—", "—", "—", "—", "—"),
            (
                "Sur", "Breach", "DIA 1", "1.09", "50%", "DVVDV", "23%", "142",
            ),
            (
                "Marea", "Omen", "DIA 2", "1.14", "55%", "VDVDV", "25%", "148",
            ),
            (
                "Ámbar", "Sova", "PLA 3", "0.96", "50%", "DDVVD", "20%", "129",
            ),
        ];
        Self {
            players: rows
                .into_iter()
                .enumerate()
                .map(|(i, (name, agent, rank, kd, wr, form, hs, adr))| Player {
                    name,
                    agent,
                    rank,
                    kd,
                    wr,
                    form,
                    hs,
                    adr,
                    hidden: i == 6,
                })
                .collect(),
            rounds: vec![
                (1, Some((1, 1))),
                (2, Some((0, 1))),
                (3, Some((2, 0))),
                (4, Some((1, 1))),
                (5, Some((4, 0))),
                (6, Some((0, 1))),
                (7, None),
            ],
            matches: vec![
                Match {
                    map: "Ascent",
                    agent: "Sova",
                    score: "13:9",
                    won: true,
                    kills: 27,
                    deaths: 15,
                    assists: 8,
                    rr: 19,
                    acs: 264,
                },
                Match {
                    map: "Haven",
                    agent: "Omen",
                    score: "8:13",
                    won: false,
                    kills: 14,
                    deaths: 18,
                    assists: 6,
                    rr: -16,
                    acs: 186,
                },
                Match {
                    map: "Bind",
                    agent: "Sova",
                    score: "13:11",
                    won: true,
                    kills: 18,
                    deaths: 19,
                    assists: 10,
                    rr: 17,
                    acs: 209,
                },
            ],
            post: false,
        }
    }
}
