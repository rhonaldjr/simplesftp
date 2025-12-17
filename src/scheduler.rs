use crate::settings::{ScheduleConfig, ScheduleMode, WeekDays};
use chrono::{DateTime, Datelike, Duration, Local, Timelike, Weekday};

pub struct Scheduler;

impl Scheduler {
    pub fn is_allowed(config: &ScheduleConfig, now: DateTime<Local>) -> bool {
        match config.mode {
            ScheduleMode::None => true,
            ScheduleMode::Daily => Self::check_time(config, now),
            ScheduleMode::Weekly => Self::check_weekly(config, now),
        }
    }

    fn check_time(config: &ScheduleConfig, now: DateTime<Local>) -> bool {
        let current_minutes = now.hour() as u32 * 60 + now.minute() as u32;
        let start_minutes = config.start_time.hour as u32 * 60 + config.start_time.minute as u32;
        let end_minutes = config.end_time.hour as u32 * 60 + config.end_time.minute as u32;

        if start_minutes == end_minutes {
            return true;
        }

        if start_minutes < end_minutes {
            // Normal day range
            current_minutes >= start_minutes && current_minutes < end_minutes
        } else {
            // Overnight range
            current_minutes >= start_minutes || current_minutes < end_minutes
        }
    }

    fn check_weekly(config: &ScheduleConfig, now: DateTime<Local>) -> bool {
        let current_minutes = now.hour() as u32 * 60 + now.minute() as u32;
        let start_minutes = config.start_time.hour as u32 * 60 + config.start_time.minute as u32;
        let end_minutes = config.end_time.hour as u32 * 60 + config.end_time.minute as u32;

        if start_minutes == end_minutes {
            // If full day, just check today
            return Self::check_day_enabled(&config.days, now.weekday());
        }

        if start_minutes < end_minutes {
            // Normal day range: Must be allowed today AND in time range
            if current_minutes >= start_minutes && current_minutes < end_minutes {
                Self::check_day_enabled(&config.days, now.weekday())
            } else {
                false
            }
        } else {
            // Overnight range
            if current_minutes >= start_minutes {
                // Evening side: use Today's permission
                Self::check_day_enabled(&config.days, now.weekday())
            } else if current_minutes < end_minutes {
                // Morning side: use Yesterday's permission
                let yesterday = now - Duration::days(1);
                Self::check_day_enabled(&config.days, yesterday.weekday())
            } else {
                false
            }
        }
    }

    fn check_day_enabled(days: &WeekDays, weekday: Weekday) -> bool {
        match weekday {
            Weekday::Mon => days.mon,
            Weekday::Tue => days.tue,
            Weekday::Wed => days.wed,
            Weekday::Thu => days.thu,
            Weekday::Fri => days.fri,
            Weekday::Sat => days.sat,
            Weekday::Sun => days.sun,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{TimeOfDay, WeekDays};
    use chrono::TimeZone;

    fn make_config(
        mode: ScheduleMode,
        start_h: u8,
        start_m: u8,
        end_h: u8,
        end_m: u8,
        days: Option<WeekDays>,
    ) -> ScheduleConfig {
        ScheduleConfig {
            mode,
            start_time: TimeOfDay {
                hour: start_h,
                minute: start_m,
            },
            end_time: TimeOfDay {
                hour: end_h,
                minute: end_m,
            },
            days: days.unwrap_or(WeekDays {
                mon: true,
                tue: true,
                wed: true,
                thu: true,
                fri: false,
                sat: false,
                sun: false,
            }),
        }
    }

    #[test]
    fn test_none_mode() {
        let config = make_config(ScheduleMode::None, 0, 0, 0, 0, None);
        let now = Local.with_ymd_and_hms(2023, 10, 27, 12, 0, 0).unwrap(); // Fri
        assert!(Scheduler::is_allowed(&config, now));
    }

    #[test]
    fn test_daily_normal_range() {
        let config = make_config(ScheduleMode::Daily, 9, 0, 17, 0, None);

        // 8:59 -> False
        let t1 = Local.with_ymd_and_hms(2023, 10, 27, 8, 59, 0).unwrap();
        assert!(!Scheduler::is_allowed(&config, t1));

        // 9:00 -> True
        let t2 = Local.with_ymd_and_hms(2023, 10, 27, 9, 0, 0).unwrap();
        assert!(Scheduler::is_allowed(&config, t2));

        // 16:59 -> True
        let t3 = Local.with_ymd_and_hms(2023, 10, 27, 16, 59, 0).unwrap();
        assert!(Scheduler::is_allowed(&config, t3));

        // 17:00 -> False
        let t4 = Local.with_ymd_and_hms(2023, 10, 27, 17, 0, 0).unwrap();
        assert!(!Scheduler::is_allowed(&config, t4));
    }

    #[test]
    fn test_daily_overnight_range() {
        // 22:00 to 05:00
        let config = make_config(ScheduleMode::Daily, 22, 0, 5, 0, None);

        // 21:59 -> False
        let t1 = Local.with_ymd_and_hms(2023, 10, 27, 21, 59, 0).unwrap();
        assert!(!Scheduler::is_allowed(&config, t1));

        // 23:00 -> True
        let t2 = Local.with_ymd_and_hms(2023, 10, 27, 23, 0, 0).unwrap();
        assert!(Scheduler::is_allowed(&config, t2));

        // 02:00 -> True
        let t3 = Local.with_ymd_and_hms(2023, 10, 27, 2, 0, 0).unwrap();
        assert!(Scheduler::is_allowed(&config, t3));

        // 05:00 -> False
        let t4 = Local.with_ymd_and_hms(2023, 10, 27, 5, 0, 0).unwrap();
        assert!(!Scheduler::is_allowed(&config, t4));
    }

    #[test]
    fn test_weekly_logic() {
        // Enabled: Mon, Tue, Wed, Thu.
        // Disabled: Fri, Sat, Sun.
        // Time: 23:00 to 02:00 (Overnight).
        let config = make_config(ScheduleMode::Weekly, 23, 0, 2, 0, None);

        // Thu 23:30 (Thu is enabled) -> Should be True
        let thu_night = Local.with_ymd_and_hms(2023, 10, 26, 23, 30, 0).unwrap(); // Oct 26 2023 is Thu
        assert!(Scheduler::is_allowed(&config, thu_night));

        // Fri 01:30 (Fri is disabled, but this is "Thursday night" part, Thu is enabled) -> Should be True
        let fri_morning = Local.with_ymd_and_hms(2023, 10, 27, 1, 30, 0).unwrap(); // Oct 27 2023 is Fri
        assert!(Scheduler::is_allowed(&config, fri_morning));

        // Fri 23:30 (Fri is disabled) -> Should be False
        let fri_night = Local.with_ymd_and_hms(2023, 10, 27, 23, 30, 0).unwrap();
        assert!(!Scheduler::is_allowed(&config, fri_night));

        // Sat 01:30 (Sat disabled, Friday night was disabled) -> Should be False
        let sat_morning = Local.with_ymd_and_hms(2023, 10, 28, 1, 30, 0).unwrap();
        assert!(!Scheduler::is_allowed(&config, sat_morning));
    }
}
