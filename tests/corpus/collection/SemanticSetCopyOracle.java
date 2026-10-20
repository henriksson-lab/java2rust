class SemanticSetCopyOracle {
    public static void main(String[] args) {
        int result = SemanticSetCopy.run();
        if (result != 0) throw new AssertionError(result);
        System.out.println(result);
    }
}
