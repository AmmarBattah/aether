//! Application schedules.
use aether_core::event::{Event, EventKind};
use aether_core::id::ScheduleId;
use aether_core::inbox::InboxKind;

use super::{App, Command, Effect, Then, transcript};

impl App {
    // ----- replies ------------------------------------------------------------------

    pub(super) fn remember_usage(
        &mut self,
        provider: &str,
        report: aether_core::provider::ProviderUsage,
    ) {
        self.quotas
            .entry(provider.to_owned())
            .and_modify(|latest| {
                if report.observed_at >= latest.observed_at {
                    *latest = report.clone();
                }
            })
            .or_insert(report);
    }

    pub(super) fn usage_request(&mut self) -> Effect {
        self.usage_loading = true;
        self.usage_error = None;
        self.dirty = true;
        Effect::Request {
            command: Command::Providers,
            then: Then::Usage,
        }
    }

    pub(super) fn report_request(&mut self, run: aether_core::id::RunId) -> Effect {
        self.report.run = Some(run);
        self.report.report = None;
        self.report.loading = true;
        self.report.error = None;
        Effect::Request {
            command: Command::GetRunReport { run },
            then: Then::RunReport(run),
        }
    }

    pub(super) fn schedules_request(&mut self) -> Effect {
        self.schedules.loading = true;
        self.schedules.refresh_pending = false;
        self.schedules.error = None;
        Effect::Request {
            command: Command::ListSchedules { target: None },
            then: Then::Schedules,
        }
    }

    /// Remember each missing schedule once, even when a list reply omits it or fails.
    pub(super) fn note_unlisted_schedule(&mut self, event: &Event) -> bool {
        let EventKind::InboxDelivered { item } = &event.kind else {
            return false;
        };
        let InboxKind::ExternalTrigger { source, .. } = &item.kind else {
            return false;
        };
        let Some(id) = transcript::schedule_of(source) else {
            return false;
        };
        !self.schedules.name_lookups.contains(&id)
            && !self.schedules.jobs.iter().any(|job| job.id == id)
            && self.schedules.name_lookups.insert(id)
    }

    /// Lists the schedules so a delivery's cell can name them, unless a list is on its way.
    pub(super) fn schedule_names_request(&mut self) -> Option<Effect> {
        (!self.schedules.loading).then(|| self.schedules_request())
    }

    /// A history reply still applies: the view shows history only for a listed job.
    pub(super) fn wants_schedule_history(&self, id: ScheduleId) -> bool {
        self.schedules.jobs.iter().any(|job| job.id == id)
    }

    /// After a list reply for the `/schedules` view: run a refresh asked for
    /// meanwhile, else load the first job's history unless shown history still applies.
    pub(super) fn after_schedules_for_view(&mut self) -> Vec<Effect> {
        if self.schedules.refresh_pending {
            return vec![self.schedules_request()];
        }
        let jobs = &self.schedules.jobs;
        let shown = self
            .schedules
            .history
            .as_ref()
            .is_some_and(|(id, _)| jobs.iter().any(|job| job.id == *id));
        if shown {
            return Vec::new();
        }
        jobs.first()
            .map(|job| history_request(job.id))
            .into_iter()
            .collect()
    }
}

fn history_request(id: ScheduleId) -> Effect {
    Effect::Request {
        command: Command::ScheduleHistory { id, limit: 100 },
        then: Then::ScheduleHistory(id),
    }
}
