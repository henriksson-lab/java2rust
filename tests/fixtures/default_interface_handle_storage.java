interface StoredView<T> {}

class EmptyStoredView<T> implements StoredView<T> {}

class WrappedStoredView<T> implements StoredView<T> {
    private final StoredView<T> value;
    WrappedStoredView(StoredView<T> value) { this.value = value; }
}

interface ViewWrapper<T> {
    default StoredView<T> wrap(StoredView<T> value) {
        return new WrappedStoredView<T>(value);
    }
}

class DefaultViewWrapper<T> implements ViewWrapper<T> {}
