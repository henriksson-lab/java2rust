package client;
public class ReaderUse {
    public static String run(api.Scope.Reader<String> reader) {
        return reader.read("ok");
    }
}
