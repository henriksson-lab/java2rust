class CheckedVirtualStringsOracle {
    public static void main(String[] args) {
        for (int mode = 0; mode < 4; mode++) {
            String expected = mode == 0 ? "ok,ok" : mode == 1 ? "caught" : mode == 2 ? "ok,null" : "null receiver";
            if (!CheckedVirtualStrings.direct(mode).equals(expected)) throw new AssertionError("direct " + mode);
            if (!CheckedVirtualStrings.consumeDeferred(mode).equals(expected)) throw new AssertionError("deferred " + mode);
            if (!CheckedVirtualStrings.afterCall(mode).equals(mode == 3 ? "bad argument" : expected)) throw new AssertionError("checked " + mode);
        }
        if (!CheckedVirtualStrings.deferredOnly().equals("created")) throw new AssertionError("eager callback");
        System.out.println("ok,ok:caught:ok,null:null receiver:created");
    }
}
