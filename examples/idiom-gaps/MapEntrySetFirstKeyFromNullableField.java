import java.util.Map;

class MapEntrySetFirstKeyFromNullableField {
    Holder holder;

    String firstKey() {
        return holder.getValues().entrySet().iterator().next().getKey();
    }
}

class Holder {
    Map<String, String> getValues() {
        return null;
    }
}
