use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use std::cmp::Ordering;

#[derive(Debug, Clone)]
pub struct HighScore {
    pub name: String,
    pub score: i64,
    pub when: DateTime<Utc>,
}

impl HighScore {
    pub fn new(name: &str, score: i64, when: DateTime<Utc>) -> HighScore {
        HighScore {
            name: name.to_owned(),
            score,
            when,
        }
    }
}

impl PartialEq<HighScore> for HighScore {
    fn eq(&self, other: &HighScore) -> bool {
        other.name == self.name && other.score == self.score
    }
}

impl PartialOrd for HighScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.score > other.score {
            return Some(Ordering::Greater);
        }

        if self.score < other.score {
            return Some(Ordering::Less);
        }

        Some(Ordering::Equal)
    }
}

impl Eq for HighScore {}

impl Ord for HighScore {
    fn cmp(&self, other: &Self) -> Ordering {
        if self < other {
            return Ordering::Less;
        }

        if self > other {
            return Ordering::Greater;
        }

        Ordering::Equal
    }

    fn max(self, other: Self) -> Self
    where
        Self: Sized,
    {
        if self > other {
            return self;
        }

        other
    }

    fn min(self, other: Self) -> Self
    where
        Self: Sized,
    {
        if self < other {
            return self;
        }

        self
    }

    fn clamp(self, min: Self, max: Self) -> Self
    where
        Self: Sized,
    {
        if self < min {
            return min;
        }

        if self > max {
            return max;
        }

        self
    }
}

/// Tracks the top game scores.
pub trait Scoreboard {
    /// Add a new high score to the scoreboard.
    ///
    /// Returns `Ok(true)` if the score was added to the scoreboard, `Ok(false)` if the score was
    /// not good enough to make the scoreboard.
    fn add(&mut self, who: &str, score: i64) -> Result<bool>;

    /// Get the best top score if one exists.
    fn first(&self) -> Option<HighScore>;

    /// Get the worst top score if one exists.
    fn last(&self) -> Option<HighScore>;

    /// Get all high scores.
    fn all(&self) -> &[HighScore];
}

/// An in-memory Scoreboard.
#[derive(Debug)]
pub struct MinimalScoreboard {
    high_scores: Vec<HighScore>,
}

impl MinimalScoreboard {
    /// Construct a new Scoreboard with the top `n` players.
    pub fn new(n: usize) -> Self {
        MinimalScoreboard {
            high_scores: Vec::with_capacity(n),
        }
    }

    /// Initialize from a pre-existing set of `HighScores`.
    pub fn init(n: usize, to_load: Vec<HighScore>) -> Self {
        let mut sb = Self::new(n);
        sb.high_scores = to_load.into_iter().take(n).collect();
        sb.high_scores.sort_unstable_by(|a, b| b.cmp(a));
        sb
    }
}

impl Scoreboard for MinimalScoreboard {
    fn add(&mut self, who: &str, score: i64) -> Result<bool> {
        let utc_now = Utc::now();

        if let Some(worst) = self.last() {
            if worst.score > score {
                return Ok(false);
            }
        }

        if self.high_scores.capacity() == self.high_scores.len() {
            self.high_scores.pop();
        }
        self.high_scores.push(HighScore::new(who, score, utc_now));
        self.high_scores.sort_unstable_by(|a, b| b.cmp(a));
        Ok(true)
    }

    fn first(&self) -> Option<HighScore> {
        if self.high_scores.is_empty() {
            return None;
        }

        Some(self.high_scores[0].clone())
    }

    fn last(&self) -> Option<HighScore> {
        if self.high_scores.is_empty() {
            return None;
        }

        Some(self.high_scores[self.high_scores.len() - 1].clone())
    }

    fn all(&self) -> &[HighScore] {
        self.high_scores.as_slice()
    }
}

impl Default for MinimalScoreboard {
    fn default() -> Self {
        MinimalScoreboard::new(5)
    }
}

#[derive(Debug)]
pub struct LocalScoreBoard {
    internal: MinimalScoreboard,
    db_conn: Connection,
}

impl LocalScoreBoard {
    pub fn new<P>(n: usize, connection_string: P) -> Result<Self>
    where
        P: AsRef<std::path::Path>,
    {
        let db_conn = Connection::open(connection_string)?;
        db_conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS scoreboard (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                score INTEGER NOT NULL,
                "when" TEXT NOT NULL
            )"#,
            (),
        )?;

        // Ensure the database only contains the top 'n' scores.
        db_conn.execute(
            r#"
            DELETE FROM scoreboard 
            WHERE rowid NOT IN (
                SELECT rowid 
                FROM scoreboard 
                ORDER BY score DESC 
                LIMIT (?)
            );
        "#,
            [n],
        )?;

        let found: Vec<_> = db_conn
            .prepare("SELECT * FROM scoreboard ORDER BY score DESC LIMIT (?1)")?
            .query_map([n], |row| {
                let maybe_date: String = row.get(3)?;
                Ok(HighScore {
                    name: row.get(1)?,
                    score: row.get(2)?,
                    when: DateTime::parse_from_rfc3339(&maybe_date)
                        .map_err(|_| {
                            rusqlite::Error::InvalidColumnType(
                                3,
                                "when".to_string(),
                                rusqlite::types::Type::Text,
                            )
                        })?
                        .with_timezone(&Utc),
                })
            })?
            .map(|item| item.unwrap())
            .collect();

        let internal = MinimalScoreboard::init(n, found);

        Ok(Self { internal, db_conn })
    }
}

impl Scoreboard for LocalScoreBoard {
    fn add(&mut self, who: &str, score: i64) -> Result<bool> {
        let last = self.internal.last();
        let added = self.internal.add(who, score);
        if let Ok(false) = added {
            return Ok(false);
        }

        if let Some(worst_score) = last {
            self.db_conn.execute(
                r#"
                DELETE FROM scoreboard WHERE
                    name = (?) AND
                    score = (?) AND
                    "when" = (?)
            "#,
                params![
                    worst_score.name,
                    worst_score.score,
                    worst_score.when.to_rfc3339()
                ],
            )?;
        }

        self.db_conn.execute(
            r#"
            INSERT INTO scoreboard (name, score, "when")
            VALUES ((?), (?), (?))
        "#,
            params![who, score, Utc::now().to_rfc3339()],
        )?;

        Ok(true)
    }

    fn first(&self) -> Option<HighScore> {
        self.internal.first()
    }

    fn last(&self) -> Option<HighScore> {
        self.internal.last()
    }

    fn all(&self) -> &[HighScore] {
        self.internal.all()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn score_comparison() {
        let a = HighScore::new(
            "Allison",
            100,
            Utc.with_ymd_and_hms(2001, 01, 01, 0, 0, 0).unwrap(),
        );
        let b = HighScore::new(
            "Bob",
            90,
            Utc.with_ymd_and_hms(2002, 02, 02, 0, 0, 0).unwrap(),
        );
        let c = HighScore::new(
            "Bob",
            90,
            Utc.with_ymd_and_hms(2003, 01, 02, 0, 0, 0).unwrap(),
        );

        assert_ne!(a, b);
        assert_eq!(b, c);
    }

    #[test]
    fn scoreboard_add() {
        let mut sb = MinimalScoreboard::new(3);
        sb.add("Allison", 2).unwrap();
        sb.add("Bob", 1).unwrap();
        sb.add("Charlie", 3).unwrap();
        sb.add("David", 4).unwrap();

        assert_eq!(sb.high_scores.len(), 3);
        match sb.first() {
            Some(high_score) => {
                assert_eq!(high_score.score, 4);
                assert_eq!(high_score.name, "David");
            }
            None => {
                assert!(false);
            }
        }

        sb.add("Eddie", 10).unwrap();
        assert_eq!(sb.high_scores.len(), 3);
        match sb.first() {
            Some(high_score) => {
                assert_eq!(high_score.score, 10);
                assert_eq!(high_score.name, "Eddie");
            }
            None => {
                assert!(false);
            }
        }
    }
}
