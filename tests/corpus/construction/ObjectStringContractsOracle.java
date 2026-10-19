class ObjectStringContractsOracle {
    public static void main(String[] args) {
        if (!ObjectStringContracts.nativeBuilder(true).equals("value")) throw new AssertionError("builder");
        if (!ObjectStringContracts.nativeBuilder(false).equals("absent")) throw new AssertionError("builder branch");
        if (!ObjectStringContracts.nativeBuffer().equals("BUFFER")) throw new AssertionError("buffer");
        if (!ObjectStringContracts.nativeRandom()) throw new AssertionError("native display");
        if (!ObjectStringContracts.nullableBranch()) throw new AssertionError("nullable branch");
        if (!ObjectStringContracts.presentBranch().equals("present")) throw new AssertionError("present branch");
        if (!ObjectStringContracts.absentBranch().equals("missing")) throw new AssertionError("absent branch");
        if (!ObjectStringContracts.boxedNullResult()) throw new AssertionError("boxed result");
        if (!ObjectStringContracts.boxedPresentResult()) throw new AssertionError("boxed present result");
        String original = new String("cached");
        if (new NullableDescription(original).toString(false) != original) throw new AssertionError("nullable field reference");
        System.out.println("value:absent:BUFFER:true:true:present:missing:true");
    }
}
