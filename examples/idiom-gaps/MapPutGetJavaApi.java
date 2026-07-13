class MapPutGetJavaApi {
    String replace(Map<String, String> values, String key, String value) {
        values.put(key, value);
        return values.get(key);
    }
}

