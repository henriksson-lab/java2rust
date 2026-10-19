public class CheckedStreamJoiningOracle {
    public static void main(String[] args) {
        String[] actual = {CheckedStreamJoining.run(false), CheckedStreamJoining.run(true),
            CheckedStreamJoining.empty(), CheckedStreamJoining.simple(), CheckedStreamJoining.nested(),
            CheckedStreamJoining.argumentFailure(), CheckedStreamJoining.argumentMutation(),
            CheckedStreamJoining.fixedListPolicy(), CheckedStreamJoining.nullNested(), CheckedStreamJoining.plainNested(), CheckedStreamJoining.aliasNestedList()};
        String[] expected = {"[a,b]:12344", "caught:12344", "[]:123", "a,b:44", "g[ga,gb]:12344", "argument:5", "[a,b]:144", "z:2", "null:0", "g[ga,gb]:12344", "z"};
        if (!java.util.Arrays.equals(actual, expected)) throw new AssertionError(java.util.Arrays.toString(actual));
        System.out.println(java.util.Arrays.toString(actual));
    }
}
