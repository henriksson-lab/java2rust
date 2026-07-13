package idiom.gaps;

public class TypedGetterAvoidsDowncast {
    public String readName(ExternalBox box) {
        String name = box.getName();
        return name;
    }
}
