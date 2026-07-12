#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RewriteNeed {
    LoopContinueUpdatePath,
    BorrowConflict,
    EscapingInnerCapture,
    ComplexFinally,
}
