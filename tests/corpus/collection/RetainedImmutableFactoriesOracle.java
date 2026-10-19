public class RetainedImmutableFactoriesOracle {
    public static void main(String[] args) {
        System.out.println(RetainedImmutableFactories.sets());
        System.out.println(RetainedImmutableFactories.lists());
        System.out.println(RetainedImmutableFactories.nested());
        System.out.println(RetainedImmutableFactories.unique());
        System.out.println(RetainedImmutableFactories.nullable());
        System.out.println(RetainedImmutableFactories.effects());
        System.out.println(RetainedImmutableFactories.genericReuse());
    }
}
