# flip-flop-data

This is a simple packet format for communicating flip-flop-app commands and events over a network. The "data" component refers to the OSI data-link layer.

The packet format incorporates AES-128 CCM encryption, thereby providing authentication and validation of the message.

Please refer to the module's tests for an illustration of usage.