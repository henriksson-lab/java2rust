class SharedConstructorAbiOracle {
    public static void main(String[] args) {
        if (SharedConstructorAbi.run() != 78) {
            throw new AssertionError("source interface lost constructor state");
        }
        if (SharedConstructorAbi.assigned() != 3) {
            throw new AssertionError("nullable assignment lost constructor state");
        }
        System.out.println("shared-constructor-abi: ok");
    }
}
