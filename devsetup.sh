OPWD="${PWD}"
cd /home/dk/dev/openwrt
export STAGING_DIR="$PWD/staging_dir"
export PATH="$PWD/$(echo staging_dir/toolchain-*/bin):$PATH"
CC=mips-openwrt-linux-gcc
cd ${OPWD}
