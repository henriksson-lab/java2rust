class NullCheckedBorrowsOracle {
    public static void main(String[] args) {
        int result = NullCheckedBorrows.run();
        if (result != 0) throw new AssertionError(result);
        System.out.println(result);
    }
}
