class RetainedArrayRowsOracle {
    public static void main(String[] args) {
        if (!ErasedRowRead.run()) throw new AssertionError("erased inspection");
        if (!ErasedRowRead.cloneRun()) throw new AssertionError("erased clone");
        if (!ErasedRowRead.leafCloneRun()) throw new AssertionError("erased leaf clone");
        if (!NullableArrayRows.absentResult() || !NullableArrayRows.caughtResult()) throw new AssertionError("nullable rows");
        if (!RetainedArrayRows.aliasResult().equals("old:null:null")) throw new AssertionError("row alias");
        if (!RetainedArrayRows.cloneResult().equals("shared:null")) throw new AssertionError("shallow clone");
        if (!RetainedArrayRows.callResult().equals("call:direct")) throw new AssertionError("row calls");
        if (RetainedArrayRows.negativeDimension() != 9) throw new AssertionError("inner dimension");
        System.out.println("true:old:null:null:shared:null");
    }
}
