class ReusableCallbacksOracle {
    public static void main(String[] args) {
        int result = ReusableCallbacks.run();
        if (result != 22) throw new AssertionError("reusable captures");
        System.out.println(result);
    }
}
