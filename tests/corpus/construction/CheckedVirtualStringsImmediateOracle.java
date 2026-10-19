class CheckedVirtualStringsImmediateOracle {
    public static void main(String[] args) {
        for (int mode = 0; mode < 4; mode++) {
            String expected = mode == 0 ? "ok,ok" : mode == 1 ? "caught" : mode == 2 ? "ok,null" : "null receiver";
            if (!CheckedVirtualStrings.direct(mode).equals(expected)) throw new AssertionError("direct " + mode);
            if (!CheckedVirtualStrings.reference(mode).equals(expected)) throw new AssertionError("reference " + mode);
            if (!CheckedVirtualStrings.afterCall(mode).equals(mode == 3 ? "bad argument" : expected)) throw new AssertionError("checked " + mode);
        }
        System.out.println("ok,ok:caught:ok,null:null receiver:bad argument");
    }
}
