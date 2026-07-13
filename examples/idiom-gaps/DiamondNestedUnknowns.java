class DiamondNestedUnknowns {
    Map<String, List<Integer>> byName(List<Integer> values) {
        Map<String, List<Integer>> out = new HashMap<>();
        out.put("values", new ArrayList<>(values));
        return out;
    }
}
