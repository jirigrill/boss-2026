# Create ram disk
sudo mkdir -p /mnt/tmp
sudo mount -t tmpfs -o size=4g tmpfs /mnt/tmp/

# Copy original test files into user directory
git clone https://github.com/bitcoin/bitcoin --depth 1 ./origin
cd origin
git fetch --depth 1 origin d0f6d9953a15d7c7111d46dcb76ab2bb18e5dee3
git checkout d0f6d9953a15d7c7111d46dcb76ab2bb18e5dee3
cp -r test ../bitcoin
cd ..

# Run original test against their code
python3 bitcoin/build/test/functional/test_runner.py --cachedir=/mnt/tmp/cache --tmpdir=/mnt/tmp --jobs $(nproc) -r results.csv || echo "test_runner completed with non-zero exit code"