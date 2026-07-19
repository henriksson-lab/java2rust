class MapForEachBlockLambda {
    void copy(Map<String, Item> source, Map<String, Item> dest) {
        source.forEach((name, item) -> {
            Item copied = item.copy();
            dest.put(name, copied);
        });
    }
}

class Item {
    Item copy() {
        return this;
    }
}
