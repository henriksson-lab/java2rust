public class RetainedBulkOracle {
    public static void main(String[] args) {
        System.out.println(RetainedBulk.run());
        System.out.println(RetainedBulk.nullSource());
        System.out.println(RetainedBulk.emptyInvalidation());
        System.out.println(RetainedBulk.receiverReplacement());
        System.out.println(RetainedBulk.sameOwnerReplacement());
    }
}
