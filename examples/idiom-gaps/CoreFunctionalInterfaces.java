class CoreFunctionalInterfaces {
    List<String> applyAll(List<Integer> values, Predicate<Integer> keep, Function<Integer, String> render) {
        List<String> out = new ArrayList<>();
        for (Integer value : values) {
            if (keep.test(value)) {
                out.add(render.apply(value));
            }
        }
        return out;
    }

    String create(Supplier<String> supplier) {
        return supplier.get();
    }
}
