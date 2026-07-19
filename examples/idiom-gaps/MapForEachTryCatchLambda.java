class MapForEachTryCatchLambda {
    void copy(Map<String, Future> futures, Map<String, Item> dest) {
        futures.forEach((name, future) -> {
            try {
                Item item = future.get();
                dest.put(name, item);
            } catch (IOException ex) {
                throw new UserException("failed", ex);
            }
        });
    }
}

class Future {
    Item get() throws IOException {
        return new Item();
    }
}

class Item {
}

class IOException extends Exception {
}
