public final class AbstractDeclarationContractsOracle {
    public static void main(String[] args) {
        int result = AbstractDeclarationContracts.run();
        if (result != 0) throw new AssertionError("abstract declaration contract " + result);
        System.out.println(result);
    }
}
