class ChainedReferenceAssignmentsOracle {
    public static void main(String[] args) {
        if (!ChainedReferenceAssignments.clear()) throw new AssertionError("clear");
        if (ChainedReferenceAssignments.fill() != 14) throw new AssertionError("alias");
        if (!ChainedReferenceAssignments.strings()) throw new AssertionError("string identity");
        System.out.println("true:14:true");
    }
}
