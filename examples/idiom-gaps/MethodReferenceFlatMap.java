class MethodReferenceFlatMap {
    Set<String> flatten(List<List<String>> groups) {
        return groups.stream().flatMap(Collection::stream).collect(Collectors.toSet());
    }
}

