public class GenericInterfaceConstructionOracle {
    public static void main(String[] arguments) {
        SnapshotView<String> snapshot = GenericInterfaceConstruction.snapshot();
        assert snapshot.values().isEmpty();
        MembershipView<String> membership = GenericInterfaceConstruction.membership();
        assert !membership.contains("absent");
        assert GenericInterfaceConstruction.<String>concrete().values().isEmpty();
        assert GenericInterfaceConstruction.<String>empty() != null;
        assert GenericInterfaceConstruction.<String, Integer>mixed().values().isEmpty();
        assert GenericInterfaceConstruction.<String>local().values().isEmpty();
        assert GenericInterfaceConstruction.assigned(snapshot).values().isEmpty();
        assert GenericInterfaceConstruction.<String>enveloped() != null;
        assert new OwnerInterfaceConstruction<String>().snapshot().values().isEmpty();
        assert GenericInterfaceConstruction.<String>argument().values().isEmpty();
        assert GenericInterfaceConstruction.<String>forwarded().values().isEmpty();
        assert GenericInterfaceConstruction.<String>cast().values().isEmpty();
        for (boolean first : new boolean[] {false, true}) {
            assert GenericInterfaceConstruction.<String>conditional(first).values().isEmpty();
            assert GenericInterfaceConstruction.<String>nested(first).values().isEmpty();
        }
        System.out.println("generic-interface-construction: ok");
    }
}
