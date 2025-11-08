{
  "targets": [
    {
      "target_name": "anidb_client",
      "cflags!": [ "-fno-exceptions" ],
      "cflags_cc!": [ "-fno-exceptions" ],
      "sources": [
        "src/native/anidb_client.cc",
        "src/native/client_wrapper.cc",
        "src/native/async_worker.cc",
        "src/native/stream_worker.cc",
        "src/native/utils.cc"
      ],
      "include_dirs": [
        "<!@(node -p \"require('node-addon-api').include\")",
        "../../anidb_client_core/include"
      ],
      "libraries": [
        "<(module_root_dir)/../../target/release/libanidb_client_core.a"
      ],
      "dependencies": [
        "<!(node -p \"require('node-addon-api').gyp\")"
      ],
      "defines": [ "NAPI_DISABLE_CPP_EXCEPTIONS" ],
      "conditions": [
        ["OS=='win'", {
          "libraries": [
            "<(module_root_dir)/../../target/release/anidb_client_core.lib"
          ],
          "msvs_settings": {
            "VCCLCompilerTool": {
              "ExceptionHandling": 1
            }
          }
        }],
        ["OS=='mac'", {
          "xcode_settings": {
            "GCC_ENABLE_CPP_EXCEPTIONS": "YES",
            "MACOSX_DEPLOYMENT_TARGET": "10.15",
            "OTHER_CFLAGS": [
              "-std=c++17"
            ]
          }
        }],
        ["OS=='linux'", {
          "cflags_cc": [
            "-std=c++17"
          ]
        }]
      ]
    }
  ]
}