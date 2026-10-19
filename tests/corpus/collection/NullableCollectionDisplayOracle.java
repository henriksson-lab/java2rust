class NullableCollectionDisplayOracle {
    public static void main(String[] args) {
        if (!NullableCollectionDisplay.probe(false).equals("list=null;set=null;map=null;<null>;A=null;B=null")
                || !NullableCollectionDisplay.probe(true).equals("list=[3, 1];set=[3, 1];map={k=5};<[3, 1]>;A=[3, 1];B=[3, 1]")
                || !NullableCollectionDisplay.fromReturn(false).equals("R=null")
                || !NullableCollectionDisplay.fromReturn(true).equals("R=[3, 1]")
                || !NullableCollectionDisplay.fromField(false).equals("F=null")
                || !NullableCollectionDisplay.fromField(true).equals("F=[3, 1]")
                || !NullableCollectionDisplay.fromMap(false).equals("M=null")
                || !NullableCollectionDisplay.fromMap(true).equals("M=[3, 1]")
                || NullableCollectionDisplay.directNull() != 8) {
            throw new AssertionError("nullable collection display contract");
        }
        System.out.println("nullable-collection-display: ok");
    }
}
