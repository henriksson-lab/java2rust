public class DefaultInterfaceHandleStorageOracle {
    public static void main(String[] arguments) {
        StoredView<String> value = new EmptyStoredView<String>();
        DefaultViewWrapper<String> concrete = new DefaultViewWrapper<String>();
        assert concrete.wrap(value) instanceof WrappedStoredView;
        ViewWrapper<String> dynamic = concrete;
        assert dynamic.wrap(value) instanceof WrappedStoredView;
        System.out.println("default-interface-handle-storage: ok");
    }
}
