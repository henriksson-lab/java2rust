import java.util.Map;

class ConstructorTrailingCallReadsSeededFieldBase {
    String key;

    ConstructorTrailingCallReadsSeededFieldBase(String key, String value) {
        this.key = key;
    }
}

class ConstructorTrailingCallReadsSeededField extends ConstructorTrailingCallReadsSeededFieldBase {
    String name;
    Map<String, String> fields;

    ConstructorTrailingCallReadsSeededField(String key, Map<String, String> mapping) {
        super(key, "");
        this.name = mapping.get("ID");
        initialize(this.name, mapping);
    }

    void initialize(String name, Map<String, String> mapping) {
        this.name = name;
        this.fields = mapping;
    }
}
