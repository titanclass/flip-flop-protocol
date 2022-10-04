# flip-flop-data

This is a simple packet format for communicating flip-flop-app commands and events over a network. The "data" component refers to the OSI data-link layer.

The packet format incorporates AES-128 CCM encryption, thereby providing authentication and validation of the message with a 4 byte MIC and 7 byte nonce.

Please refer to the module's tests for an illustration of usage.