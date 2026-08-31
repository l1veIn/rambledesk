use rambledesk_core::kernel::{AgentWorkPayload, AgentWorkRecord, RequestId};

const CONTRACT_MARKER: &str = "[RambleDesk Managed Ramble Loop]";

pub(super) fn feedback_request_id(work: &AgentWorkRecord) -> RequestId {
    RequestId::new(format!("ramble-work-{}", work.work_id))
}

pub(super) fn prompt_for_work(work: &AgentWorkRecord) -> String {
    let marker = format!("[RambleDesk work_id: {}]", work.work_id);
    let next_request_id = feedback_request_id(work);
    let task = match &work.payload {
        AgentWorkPayload::Launch {
            prompt_markdown, ..
        }
        | AgentWorkPayload::Steering {
            prompt_markdown, ..
        } => prompt_markdown.clone(),
        AgentWorkPayload::FeedbackResume {
            delivery_id,
            request_id,
        } => format!(
            "RambleDesk has resolved Feedback Request {request_id}. Call get_feedback with request_id {request_id} now. Consume the returned envelope and de-duplicate it by delivery_id {delivery_id}. Then continue the work using that feedback."
        ),
    };
    format!(
        "{marker}\n\n{task}\n\n{CONTRACT_MARKER}\nThis is a long-lived managed Ramble Session. Keep working until you need human judgment, testing, clarification, or until you have completed the current stage. Permission Requests and Ask Questions may happen while you work, but neither replaces the required final request_feedback handoff. Even when the task or current stage appears complete, report it through request_feedback before suspending. Before ending this Prompt Turn, you MUST call request_feedback exactly once with request_id `{next_request_id}` to hand the current state to the human. Never substitute a plain assistant message for request_feedback. After request_feedback succeeds, end only the current Prompt Turn; never decide that the Session is finished. RambleDesk may resume this Session much later and will explicitly ask you to call get_feedback."
    )
}

pub(super) fn protocol_repair_prompt(work: &AgentWorkRecord) -> String {
    let next_request_id = feedback_request_id(work);
    match &work.payload {
        AgentWorkPayload::FeedbackResume {
            request_id,
            delivery_id,
        } => format!(
            "[RambleDesk Protocol Repair]\n\nDo not repeat work and do not merely restate the result. First call get_feedback with request_id {request_id}, consume and apply delivery_id {delivery_id}, then call request_feedback exactly once with request_id `{next_request_id}`. Summarize the resulting stage and any decision or verification the human should provide. Re-reading the same delivery is safe; de-duplicate it by delivery_id. After request_feedback succeeds, end only this Prompt Turn; the Session remains open."
        ),
        _ => format!(
            "[RambleDesk Protocol Repair]\n\nDo not repeat the work and do not merely restate the result. The current managed Ramble Turn has not been handed back to the human. Call request_feedback exactly once now with request_id `{next_request_id}`. Summarize the result, current stage, and any decision or verification the human should provide. After request_feedback succeeds, end only this Prompt Turn; the Session remains open."
        ),
    }
}

#[cfg(test)]
mod tests {
    use rambledesk_core::kernel::{
        AgentWorkId, AgentWorkKind, AgentWorkPayload, AgentWorkRecord, AgentWorkState, DeliveryId,
        PackageId, RequestId, SessionId, SubmissionId,
    };

    use super::{feedback_request_id, prompt_for_work, protocol_repair_prompt};

    fn work(kind: AgentWorkKind, payload: AgentWorkPayload) -> AgentWorkRecord {
        AgentWorkRecord {
            work_id: AgentWorkId::new("work-1"),
            session_id: SessionId::new("session-1"),
            kind,
            source_id: "source-1".to_owned(),
            payload_digest: format!("sha256:{}", "a".repeat(64)),
            payload,
            state: AgentWorkState::Pending,
            attempt_count: 0,
            last_error_code: None,
            last_error_at: None,
            created_at: "2026-08-31T00:00:00Z".to_owned(),
            completed_at: None,
        }
    }

    #[test]
    fn every_managed_work_prompt_restates_the_ramble_loop_contract() {
        let prompts = [
            prompt_for_work(&work(
                AgentWorkKind::LaunchPrompt,
                AgentWorkPayload::Launch {
                    submission_id: SubmissionId::new("launch-1"),
                    package_id: PackageId::new("package-1"),
                    prompt_markdown: "Start the task".to_owned(),
                },
            )),
            prompt_for_work(&work(
                AgentWorkKind::SteeringPrompt,
                AgentWorkPayload::Steering {
                    submission_id: SubmissionId::new("steer-1"),
                    prompt_markdown: "Change direction".to_owned(),
                },
            )),
            prompt_for_work(&work(
                AgentWorkKind::FeedbackResume,
                AgentWorkPayload::FeedbackResume {
                    delivery_id: DeliveryId::new("delivery-1"),
                    request_id: RequestId::new("request-1"),
                },
            )),
        ];

        for prompt in prompts {
            assert!(
                prompt.contains("[RambleDesk Managed Ramble Loop]"),
                "{prompt}"
            );
            assert!(
                prompt.contains("Never substitute a plain assistant message for request_feedback"),
                "{prompt}"
            );
            assert!(
                prompt.contains("end only the current Prompt Turn"),
                "{prompt}"
            );
            assert!(
                prompt.contains("never decide that the Session is finished"),
                "{prompt}"
            );
            assert!(
                prompt.contains("request_id `ramble-work-work-1`"),
                "{prompt}"
            );
            assert!(
                prompt.contains("Permission Requests and Ask Questions")
                    && prompt.contains("neither replaces the required final request_feedback"),
                "{prompt}"
            );
            assert!(
                prompt.contains("Even when the task or current stage appears complete"),
                "{prompt}"
            );
        }
    }

    #[test]
    fn feedback_resume_reads_delivery_then_reopens_the_loop() {
        let prompt = prompt_for_work(&work(
            AgentWorkKind::FeedbackResume,
            AgentWorkPayload::FeedbackResume {
                delivery_id: DeliveryId::new("delivery-1"),
                request_id: RequestId::new("request-1"),
            },
        ));
        assert!(prompt.contains("Call get_feedback with request_id request-1 now"));
        assert!(prompt.contains("delivery_id delivery-1"));
        assert!(prompt.contains("Then continue the work using that feedback"));
        assert!(prompt.contains("request_id `ramble-work-work-1`"));
    }

    #[test]
    fn stable_request_id_is_derived_only_from_work_id() {
        let launch = work(
            AgentWorkKind::LaunchPrompt,
            AgentWorkPayload::Launch {
                submission_id: SubmissionId::new("launch-1"),
                package_id: PackageId::new("package-1"),
                prompt_markdown: "first wording".to_owned(),
            },
        );
        let steering = work(
            AgentWorkKind::SteeringPrompt,
            AgentWorkPayload::Steering {
                submission_id: SubmissionId::new("steer-1"),
                prompt_markdown: "different wording".to_owned(),
            },
        );
        assert_eq!(feedback_request_id(&launch), feedback_request_id(&steering));
        assert_eq!(feedback_request_id(&launch).as_str(), "ramble-work-work-1");
    }

    #[test]
    fn launch_prompt_does_not_depend_on_a_slash_command_or_installed_skill() {
        let prompt = prompt_for_work(&work(
            AgentWorkKind::LaunchPrompt,
            AgentWorkPayload::Launch {
                submission_id: SubmissionId::new("launch-1"),
                package_id: PackageId::new("package-1"),
                prompt_markdown: "Start the task".to_owned(),
            },
        ));
        assert!(!prompt.contains("/ramble"), "{prompt}");
        assert!(!prompt.contains("ramble skill"), "{prompt}");
    }

    #[test]
    fn repair_prompt_only_restores_the_missing_human_handoff() {
        let work = work(
            AgentWorkKind::LaunchPrompt,
            AgentWorkPayload::Launch {
                submission_id: SubmissionId::new("launch-1"),
                package_id: PackageId::new("package-1"),
                prompt_markdown: "Build the feature".to_owned(),
            },
        );
        let prompt = protocol_repair_prompt(&work);
        assert!(prompt.starts_with("[RambleDesk Protocol Repair]"));
        assert!(prompt.contains("Do not repeat the work"));
        assert!(prompt.contains("request_id `ramble-work-work-1`"));
        assert!(!prompt.contains("Build the feature"));
    }

    #[test]
    fn feedback_resume_repair_replays_the_delivery_before_reopening_the_loop() {
        let prompt = protocol_repair_prompt(&work(
            AgentWorkKind::FeedbackResume,
            AgentWorkPayload::FeedbackResume {
                delivery_id: DeliveryId::new("delivery-1"),
                request_id: RequestId::new("request-1"),
            },
        ));
        let get_position = prompt
            .find("get_feedback with request_id request-1")
            .expect("get_feedback instruction");
        let request_position = prompt
            .find("request_feedback exactly once")
            .expect("request_feedback instruction");
        assert!(get_position < request_position, "{prompt}");
        assert!(prompt.contains("delivery_id delivery-1"));
    }
}
