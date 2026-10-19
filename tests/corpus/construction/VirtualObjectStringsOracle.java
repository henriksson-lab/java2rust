class VirtualObjectStringsOracle {
    public static void main(String[] args) {
        if (!VirtualObjectStrings.nullableResult()) throw new AssertionError("lost null result");
        if (!VirtualObjectStrings.objectParameterNullResult()) throw new AssertionError("object parameter null");
        if (!VirtualObjectStrings.displayNullResult().equals("value=null")) throw new AssertionError("display");
        if (VirtualObjectStrings.checkedDispatch() != 7) throw new AssertionError("exception");
        if (!VirtualObjectStrings.genericText().equals("ok")) throw new AssertionError("generic");
        if (!VirtualObjectStrings.staticText().equals("generic")) throw new AssertionError("static");
        if (!VirtualObjectStrings.mapKey().equals("key")) throw new AssertionError("key");
        if (!VirtualObjectStrings.genericNullResult()) throw new AssertionError("generic null");
        if (VirtualObjectStrings.genericCaught() != 9) throw new AssertionError("generic exception");
        if (!VirtualObjectStrings.mutableDispatch().equals("n1:n2:2")) throw new AssertionError("object identity");
        if (!VirtualObjectStrings.typedDisplay().equals("n1:n2:2")) throw new AssertionError("typed identity");
        if (!VirtualObjectStrings.typedParameter().equals("typed=n1:1")) throw new AssertionError("typed parameter");
        if (!VirtualObjectStrings.nullableCheckedResult()) throw new AssertionError("checked null");
        if (VirtualObjectStrings.checkedNullableDispatch() != 11) throw new AssertionError("checked nullable");
        System.out.println("true:value=null:7:ok:generic:key:true:9");
    }
}
