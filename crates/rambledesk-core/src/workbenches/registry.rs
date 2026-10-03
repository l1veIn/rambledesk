register_workbenches! {
    Ramble => ramble {
        wire: "ramble", data: RambleData,
        state: [],
        result: { kind: String },
        exports: [RambleData]
    }
    Questions => questions {
        wire: "questions", data: QuestionsData,
        state: [Questions { answers: Vec<QuestionAnswer> },],
        result: { answers: Vec<QuestionAnswer>, cancelled: bool },
        exports: [QuestionsData, Question, QuestionOption, QuestionAnswer]
    }
    SingleChoice => single_choice {
        wire: "single_choice", data: SingleChoiceData,
        state: [SingleChoice { selected_option_id: Option<String> },],
        result: { status: AnswerStatus, selected_option_id: Option<String> },
        exports: [SingleChoiceData, ChoiceOption, AnswerStatus]
    }
    DocumentReview => document_review {
        wire: "document_review", data: DocumentReviewData,
        state: [DocumentReview { verdict: Option<ReviewVerdict>, annotations: Vec<ReviewAnnotation>, paragraph_marks: Vec<ParagraphMark> },],
        result: (DocumentReviewResult),
        exports: [DocumentReviewData, ReviewParagraph, ReviewAnnotation, ReviewAnnotationKind, ParagraphMark, ParagraphDecision, ReviewVerdict, DocumentReviewResult]
    }
    WebReview => web_review {
        wire: "web_review", data: WebReviewData,
        state: [WebReview { annotations: Vec<WebReviewAnnotation> },],
        result: (WebReviewResult),
        exports: [WebReviewData, WebReviewViewport, WebReviewRect, WebReviewElement, WebReviewAnnotation, WebReviewResult]
    }
    Terminal => terminal {
        wire: "terminal", data: TerminalData,
        state: [Terminal { sessions: Vec<TerminalTrialSession> },],
        result: (TerminalResult),
        exports: [TerminalData, TerminalCommand, TerminalTrialStatus, TerminalTrialSession, TerminalResult]
    }
    #[cfg(feature = "workbench-fixtures")]
    RatingReview => rating_review {
        wire: "rating_review", data: RatingReviewData,
        state: [#[cfg(feature = "workbench-fixtures")] RatingReview(RatingReviewState),],
        result: (RatingReviewResult),
        exports: [RatingReviewData, RatingReviewState, RatingReviewResult]
    }
    #[cfg(all(test, feature = "workbench-fixtures"))]
    RatingReviewTwin => rating_review_twin {
        wire: "rating_review_twin", data: RatingReviewTwinData,
        state: [#[cfg(all(test, feature = "workbench-fixtures"))] RatingReviewTwin(RatingReviewTwinState),],
        result: (RatingReviewTwinResult),
        exports: [RatingReviewTwinData, RatingReviewTwinState, RatingReviewTwinResult]
    }
    Sort => sort {
        wire: "sort", data: SortData,
        state: [Sort(SortState),],
        result: (SortResult),
        exports: [SortData, SortItem, SortLabelEdit, SortState, SortResult]
    }
    VisualFeedback => visual_feedback {
        wire: "visual_feedback", data: VisualFeedbackData,
        state: [VisualFeedback(VisualFeedbackState),],
        result: (VisualFeedbackResult),
        exports: [VisualFeedbackData, VisualFeedbackPoint, VisualFeedbackAnnotationKind, VisualFeedbackAnnotation, VisualFeedbackState, VisualFeedbackResult]
    }
    DiffReview => diff_review {
        wire: "diff_review", data: DiffReviewData,
        state: [DiffReview(DiffReviewState),],
        result: (DiffReviewResult),
        exports: [DiffReviewData, DiffReviewFile, DiffReviewSide, DiffReviewAnchor, DiffReviewComment, DiffReviewState, DiffReviewResult]
    }
    TableReview => table_review {
        wire: "table_review", data: TableReviewData,
        state: [TableReview(TableReviewState),],
        result: (TableReviewResult),
        exports: [TableReviewData, TableReviewColumn, TableReviewRow, TableReviewChange, TableReviewComment, TableReviewState, TableReviewResult]
    }
    MediaReview => media_review {
        wire: "media_review", data: MediaReviewData,
        state: [MediaReview(MediaReviewState),],
        result: (MediaReviewResult),
        exports: [MediaReviewKind, MediaReviewData, MediaReviewComment, MediaReviewState, MediaReviewResult]
    }
}
