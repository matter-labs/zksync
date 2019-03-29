{
  "targets": [
    {
      "target_name": "sha3",
      "sources": [
        "src/addon.cpp",
        "src/displayIntermediateValues.cpp",
        "src/KeccakF-1600-reference.cpp",
        "src/KeccakNISTInterface.cpp",
        "src/KeccakSponge.cpp"
      ],
      "include_dirs": [
          "<!(node -e \"require('nan')\")"
      ]
    }
  ]
}